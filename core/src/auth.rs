use crate::errors::AppError;
use crate::logging::{AuditEvent, AuditOutcome};
use axum::{extract::Request, http::header, middleware::Next, response::Response, Extension, Json};
use base64::{
    engine::general_purpose::STANDARD as BASE64,
    engine::general_purpose::URL_SAFE_NO_PAD as BASE64_URL, Engine,
};
use ed25519_dalek::{Signature as Ed25519Signature, Signer, SigningKey, Verifier, VerifyingKey};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rand::RngCore;
use rsa::{
    pkcs8::{DecodePrivateKey, EncodePrivateKey, EncodePublicKey},
    traits::PublicKeyParts,
    RsaPrivateKey, RsaPublicKey,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use soroban_sdk::xdr::{
    DecoratedSignature, Limits, ManageDataOp, Memo, MuxedAccount, Operation, OperationBody,
    Preconditions, ReadXdr, SequenceNumber, SignatureHint, TimeBounds, TimePoint, Transaction,
    TransactionEnvelope, TransactionExt, TransactionV1Envelope, Uint256, WriteXdr,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use stellar_strkey::Strkey;
use utoipa::ToSchema;
use axum::http::{HeaderMap, HeaderName, HeaderValue};
use axum::response::IntoResponse;
use std::collections::HashMap;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ApiTier {
    Free,
    Pro,
}

impl ApiTier {
    pub fn rate_limit_per_minute(&self) -> u64 {
        match self {
            ApiTier::Free => 100,
            ApiTier::Pro => 1000,
        }
    }
}

/// Length of the rate-limit window that `rate_limit_per_minute` applies to.
pub const RATE_LIMIT_WINDOW: Duration = Duration::from_secs(60);

/// Above this many tracked tenants, idle ones are evicted on the next check.
const MAX_TRACKED_RATE_LIMIT_KEYS: usize = 10_000;

/// Outcome of a rate-limit check, rendered as `X-RateLimit-*` headers (and
/// `Retry-After` when the request is throttled).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimitDecision {
    /// Whether the request fits in the window.
    pub allowed: bool,
    /// Requests allowed per window for this tenant.
    pub limit: u64,
    /// Requests left in the current window after this one.
    pub remaining: u64,
    /// Seconds until the oldest request in the window expires and frees a slot.
    pub reset_secs: u64,
}

impl RateLimitDecision {
    /// Add `X-RateLimit-Limit`, `X-RateLimit-Remaining` and `X-RateLimit-Reset`,
    /// plus `Retry-After` (in seconds) when the request was throttled.
    pub fn apply_headers(&self, headers: &mut HeaderMap) {
        let mut set = |name: &'static str, value: u64| {
            headers.insert(HeaderName::from_static(name), HeaderValue::from(value));
        };
        set("x-ratelimit-limit", self.limit);
        set("x-ratelimit-remaining", self.remaining);
        set("x-ratelimit-reset", self.reset_secs);
        if !self.allowed {
            set("retry-after", self.reset_secs.max(1));
        }
    }

    /// HTTP 429 response carrying the rate-limit headers.
    pub fn into_rejection(self) -> Response {
        let mut response = AppError::TooManyRequests(format!(
            "Rate limit of {} requests per minute exceeded",
            self.limit
        ))
        .into_response();
        self.apply_headers(response.headers_mut());
        response
    }
}

/// Sliding-window (request log) rate limiter keyed by tenant: the API key
/// hash, or the client IP for requests without one.
///
/// Unlike a fixed per-minute bucket, a burst straddling a minute boundary
/// cannot get twice the quota through.
pub struct SlidingWindowLimiter {
    window: Duration,
    requests: Mutex<HashMap<String, VecDeque<Instant>>>,
}

impl SlidingWindowLimiter {
    pub fn new(window: Duration) -> Self {
        Self {
            window,
            requests: Mutex::new(HashMap::new()),
        }
    }

    /// Record a request for `key` at `now` if it fits within `limit` requests
    /// per window, and report the resulting quota.
    pub fn check(&self, key: &str, limit: u64, now: Instant) -> RateLimitDecision {
        let window = self.window;
        let mut requests = self.requests.lock().unwrap_or_else(|e| e.into_inner());

        if requests.len() > MAX_TRACKED_RATE_LIMIT_KEYS {
            requests.retain(|_, log| {
                log.back()
                    .is_some_and(|&last| now.saturating_duration_since(last) < window)
            });
        }

        let log = requests.entry(key.to_string()).or_default();
        while let Some(&oldest) = log.front() {
            if now.saturating_duration_since(oldest) >= window {
                log.pop_front();
            } else {
                break;
            }
        }

        let allowed = (log.len() as u64) < limit;
        if allowed {
            log.push_back(now);
        }

        let reset_secs = log
            .front()
            .map(|&oldest| ceil_secs(window.saturating_sub(now.saturating_duration_since(oldest))))
            .unwrap_or(0);

        RateLimitDecision {
            allowed,
            limit,
            remaining: limit.saturating_sub(log.len() as u64),
            reset_secs,
        }
    }
}

fn ceil_secs(duration: Duration) -> u64 {
    duration.as_secs() + u64::from(duration.subsec_nanos() > 0)
}

/// Client IP from `X-Forwarded-For` (first hop) or `X-Real-IP`, if present.
fn client_ip_from_headers(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .or_else(|| headers.get("x-real-ip").and_then(|v| v.to_str().ok()))
        .map(|ip| ip.trim().to_string())
        .filter(|ip| !ip.is_empty())
}

/// Best-effort client IP for rate limiting requests that carry no API key.
fn client_ip(req: &Request) -> String {
    client_ip_from_headers(req.headers())
        .or_else(|| {
            req.extensions()
                .get::<axum::extract::ConnectInfo<std::net::SocketAddr>>()
                .map(|info| info.0.ip().to_string())
        })
        .unwrap_or_else(|| "unknown".to_string())
}

const CHALLENGE_EXPIRY_SECS: u64 = 300;
const JWT_EXPIRY_SECS: u64 = 86400;
const WEB_AUTH_DOMAIN: &str = "soroscope";

/// Constant-time byte slice comparison.
///
/// Both inputs are hashed with SHA-256 so that the comparison always operates
/// on uniform 32-byte digests, regardless of the input lengths. The digests are
/// then compared with an XOR-accumulate loop that never short-circuits, so the
/// execution time does not depend on the position of the first mismatched byte.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    let a_digest = Sha256::digest(a);
    let b_digest = Sha256::digest(b);

    let mut diff: u8 = 0;
    for (x, y) in a_digest.iter().zip(b_digest.iter()) {
        diff |= x ^ y;
    }

    diff == 0
}

/// Constant-time lookup of an API tier for a presented API key.
///
/// The presented key is hashed with SHA-256 and compared against every stored
/// key hash using [`constant_time_eq`], avoiding short-circuiting string
/// equality on the raw key.
fn lookup_api_tier(api_keys: &HashMap<String, ApiTier>, presented_key: &str) -> Option<ApiTier> {
    let presented_hash = Sha256::digest(presented_key.as_bytes());

    let mut matched: Option<ApiTier> = None;
    for (stored_hash, tier) in api_keys.iter() {
        if constant_time_eq(&presented_hash, stored_hash.as_bytes()) {
            matched = Some(tier.clone());
        }
    }

    matched
}

pub struct AuthState {
    pub encoding_key: EncodingKey,
    pub decoding_key: DecodingKey,
    pub jwk_n: String,
    pub jwk_e: String,
    pub signing_key: SigningKey,
    pub server_public_key: [u8; 32],
    pub network_passphrase: String,
    /// Emergency pause flag for message verification.
    /// When true, all verification endpoints reject requests.
    pub emergency_verification_paused: Arc<AtomicBool>,
    pub api_keys: HashMap<String, ApiTier>,
    pub rate_limiter: SlidingWindowLimiter,
}

impl AuthState {
    pub fn new(
        jwt_private_key_pem: Option<String>,
        sep10_seed: Option<[u8; 32]>,
        network_passphrase: String,
        emergency_verification_paused: bool,
    ) -> Self {
        let mut api_keys = HashMap::new();
        // Dummy stored key hashes for testing
        api_keys.insert("hash_free_key".to_string(), ApiTier::Free);
        api_keys.insert("hash_pro_key".to_string(), ApiTier::Pro);
        let rate_limiter = SlidingWindowLimiter::new(RATE_LIMIT_WINDOW);
        let seed = match sep10_seed {
            Some(seed) => seed,
            None => {
                let mut seed = [0u8; 32];
                rand::thread_rng().fill_bytes(&mut seed);
                seed
            }
        };
        let signing_key = SigningKey::from_bytes(&seed);
        let server_public_key = signing_key.verifying_key().to_bytes();

        let priv_key = if let Some(pem) = jwt_private_key_pem {
            RsaPrivateKey::from_pkcs8_pem(&pem).expect("Invalid RSA Private Key PEM")
        } else {
            tracing::info!("Generating ephemeral RSA keypair for local development...");
            let mut rng = rand::thread_rng();
            RsaPrivateKey::new(&mut rng, 2048).expect("Failed to generate RSA key")
        };

        let pem_str = priv_key.to_pkcs8_pem(rsa::pkcs8::LineEnding::LF).unwrap();
        let encoding_key = EncodingKey::from_rsa_pem(pem_str.as_bytes()).unwrap();

        let pub_key = RsaPublicKey::from(&priv_key);
        let pub_pem = pub_key
            .to_public_key_pem(rsa::pkcs8::LineEnding::LF)
            .unwrap();
        let decoding_key = DecodingKey::from_rsa_pem(pub_pem.as_bytes()).unwrap();

        let n = BASE64_URL.encode(pub_key.n().to_bytes_be());
        let e = BASE64_URL.encode(pub_key.e().to_bytes_be());

        Self {
            encoding_key,
            decoding_key,
            jwk_n: n,
            jwk_e: e,
            signing_key,
            server_public_key,
            network_passphrase,
            emergency_verification_paused: Arc::new(AtomicBool::new(emergency_verification_paused)),
            api_keys,
            rate_limiter,
        }
    }

    pub fn server_stellar_address(&self) -> String {
        Strkey::PublicKeyEd25519(stellar_strkey::ed25519::PublicKey(self.server_public_key))
            .to_string()
    }

    pub fn is_verification_paused(&self) -> bool {
        self.emergency_verification_paused.load(Ordering::SeqCst)
    }

    pub fn set_verification_paused(&self, paused: bool) {
        self.emergency_verification_paused
            .store(paused, Ordering::SeqCst);
    }
}

#[derive(Deserialize, ToSchema)]
pub struct ChallengeRequest {
    #[schema(example = "GABC...XYZ")]
    pub account: String,
}

#[derive(Serialize, ToSchema)]
pub struct ChallengeResponse {
    pub transaction: String,
    pub network_passphrase: String,
}

#[derive(Deserialize, ToSchema)]
pub struct VerifyRequest {
    pub transaction: String,
}

#[derive(Serialize, ToSchema)]
pub struct VerifyResponse {
    pub token: String,
}

/// Emergency pause toggle request (admin-only).
#[derive(Deserialize, ToSchema)]
pub struct EmergencyPauseRequest {
    /// If true, verification is paused. If false, verification resumes.
    pub paused: bool,
}

/// Emergency pause toggle response.
#[derive(Serialize, ToSchema)]
pub struct EmergencyPauseResponse {
    /// Current pause status.
    pub paused: bool,
    /// Message describing the status change.
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    sub: String,
    iss: String,
    exp: u64,
    iat: u64,
    scopes: Vec<String>,
    #[serde(default)]
    roles: Vec<String>,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn network_id(passphrase: &str) -> [u8; 32] {
    Sha256::digest(passphrase.as_bytes()).into()
}

fn tx_hash(tx: &Transaction, net_id: &[u8; 32]) -> Result<[u8; 32], AppError> {
    let tx_xdr = tx
        .to_xdr(Limits::none())
        .map_err(|e| AppError::Internal(format!("XDR encode error: {e}")))?;
    let mut h = Sha256::new();
    h.update(net_id);
    h.update(2i32.to_be_bytes()); // ENVELOPE_TYPE_TX
    h.update(&tx_xdr);
    Ok(h.finalize().into())
}

fn build_challenge_envelope(
    state: &AuthState,
    client_pubkey: &[u8; 32],
) -> Result<String, AppError> {
    let now = now_secs();

    let mut nonce = [0u8; 48];
    rand::thread_rng().fill_bytes(&mut nonce);
    let nonce_value = BASE64.encode(nonce);

    let data_name = format!("{WEB_AUTH_DOMAIN} auth");
    let manage_data = ManageDataOp {
        data_name: data_name
            .into_bytes()
            .try_into()
            .map_err(|_| AppError::Internal("data name conversion failed".into()))?,
        data_value: Some(
            nonce_value
                .into_bytes()
                .try_into()
                .map_err(|_| AppError::Internal("nonce value conversion failed".into()))?,
        ),
    };

    let op = Operation {
        source_account: Some(MuxedAccount::Ed25519(Uint256(*client_pubkey))),
        body: OperationBody::ManageData(manage_data),
    };

    let tx = Transaction {
        source_account: MuxedAccount::Ed25519(Uint256(state.server_public_key)),
        fee: 100,
        seq_num: SequenceNumber(0),
        cond: Preconditions::Time(TimeBounds {
            min_time: TimePoint(now),
            max_time: TimePoint(now + CHALLENGE_EXPIRY_SECS),
        }),
        memo: Memo::None,
        operations: vec![op]
            .try_into()
            .map_err(|_| AppError::Internal("operations conversion failed".into()))?,
        ext: TransactionExt::V0,
    };

    let net_id = network_id(&state.network_passphrase);
    let hash = tx_hash(&tx, &net_id)?;
    let sig = state.signing_key.sign(&hash);

    let hint: [u8; 4] = state.server_public_key[28..32]
        .try_into()
        .map_err(|_| AppError::Internal("invalid server public key length".into()))?;
    let decorated = DecoratedSignature {
        hint: SignatureHint(hint),
        signature: sig
            .to_bytes()
            .to_vec()
            .try_into()
            .map_err(|_| AppError::Internal("signature conversion failed".into()))?,
    };

    let envelope = TransactionEnvelope::Tx(TransactionV1Envelope {
        tx,
        signatures: vec![decorated]
            .try_into()
            .map_err(|_| AppError::Internal("envelope signatures failed".into()))?,
    });

    let xdr = envelope
        .to_xdr(Limits::none())
        .map_err(|e| AppError::Internal(format!("XDR encode error: {e}")))?;

    Ok(BASE64.encode(&xdr))
}

fn verify_challenge_envelope(state: &AuthState, signed_xdr_b64: &str) -> Result<String, AppError> {
    let raw = BASE64
        .decode(signed_xdr_b64)
        .map_err(|_| AppError::BadRequest("Invalid base64".into()))?;

    let envelope = TransactionEnvelope::from_xdr(&raw, Limits::none())
        .map_err(|_| AppError::BadRequest("Invalid transaction XDR".into()))?;

    let inner = match envelope {
        TransactionEnvelope::Tx(inner) => inner,
        _ => {
            return Err(AppError::BadRequest(
                "Expected TransactionV1 envelope".into(),
            ))
        }
    };

    let tx = &inner.tx;

    if tx.seq_num.0 != 0 {
        return Err(AppError::BadRequest("Non-zero sequence number".into()));
    }

    let source_key = match &tx.source_account {
        MuxedAccount::Ed25519(Uint256(b)) => *b,
        _ => {
            return Err(AppError::BadRequest(
                "Unsupported source account type".into(),
            ))
        }
    };
    if source_key != state.server_public_key {
        return Err(AppError::BadRequest(
            "Challenge not issued by this server".into(),
        ));
    }

    let now = now_secs();
    match &tx.cond {
        Preconditions::Time(bounds) => {
            if now > bounds.max_time.0 {
                return Err(AppError::BadRequest("Challenge expired".into()));
            }
        }
        _ => return Err(AppError::BadRequest("Missing time bounds".into())),
    }

    let ops: &[Operation] = inner.tx.operations.as_ref();
    if ops.is_empty() {
        return Err(AppError::BadRequest("No operations in challenge".into()));
    }

    let client_key = match &ops[0].source_account {
        Some(MuxedAccount::Ed25519(Uint256(b))) => *b,
        _ => {
            return Err(AppError::BadRequest(
                "Missing client account on operation".into(),
            ))
        }
    };

    match &ops[0].body {
        OperationBody::ManageData(md) => {
            let name = std::str::from_utf8(md.data_name.as_ref())
                .map_err(|_| AppError::BadRequest("Invalid data name encoding".into()))?;
            let expected = format!("{WEB_AUTH_DOMAIN} auth");
            if name != expected {
                return Err(AppError::BadRequest("Invalid manage_data key".into()));
            }
        }
        _ => return Err(AppError::BadRequest("Expected ManageData operation".into())),
    }

    let net_id = network_id(&state.network_passphrase);
    let hash = tx_hash(&inner.tx, &net_id)?;

    let sigs: &[DecoratedSignature] = inner.signatures.as_ref();
    let server_hint: [u8; 4] = state.server_public_key[28..32]
        .try_into()
        .map_err(|_| AppError::Internal("invalid server public key length".into()))?;
    let client_hint: [u8; 4] = client_key[28..32]
        .try_into()
        .map_err(|_| AppError::BadRequest("invalid client public key length".into()))?;

    let mut server_ok = false;
    let mut client_ok = false;

    for ds in sigs {
        let sig_bytes: &[u8] = ds.signature.as_ref();
        let Ok(sig) = Ed25519Signature::try_from(sig_bytes) else {
            continue;
        };

        if ds.hint.0 == server_hint {
            if let Ok(vk) = VerifyingKey::from_bytes(&state.server_public_key) {
                if vk.verify(&hash, &sig).is_ok() {
                    server_ok = true;
                }
            }
        }

        if ds.hint.0 == client_hint {
            if let Ok(vk) = VerifyingKey::from_bytes(&client_key) {
                if vk.verify(&hash, &sig).is_ok() {
                    client_ok = true;
                }
            }
        }
    }

    if !server_ok {
        return Err(AppError::Unauthorized(
            "Missing valid server signature".into(),
        ));
    }
    if !client_ok {
        return Err(AppError::Unauthorized(
            "Missing valid client signature".into(),
        ));
    }

    let client_address =
        Strkey::PublicKeyEd25519(stellar_strkey::ed25519::PublicKey(client_key)).to_string();

    let claims = Claims {
        sub: client_address,
        iss: WEB_AUTH_DOMAIN.to_string(),
        iat: now,
        exp: now + JWT_EXPIRY_SECS,
        scopes: vec!["simulate".to_string()],
        roles: vec!["user".to_string()],
    };

    let header = Header::new(Algorithm::RS256);
    encode(&header, &claims, &state.encoding_key)
        .map_err(|e| AppError::Internal(format!("JWT encode error: {e}")))
}

#[utoipa::path(
    post,
    path = "/auth/challenge",
    request_body = ChallengeRequest,
    responses(
        (status = 200, description = "SEP-10 challenge transaction", body = ChallengeResponse),
        (status = 400, description = "Invalid account"),
        (status = 503, description = "Verification paused for emergency maintenance")
    ),
    tag = "Auth"
)]
pub async fn challenge_handler(
    Extension(state): Extension<Arc<AuthState>>,
    Json(payload): Json<ChallengeRequest>,
) -> Result<Json<ChallengeResponse>, AppError> {
    if state.is_verification_paused() {
        return Err(AppError::Internal(
            "Message verification is temporarily paused for emergency maintenance".into(),
        ));
    }

    let strkey = Strkey::from_string(&payload.account)
        .map_err(|_| AppError::BadRequest("Invalid Stellar address".into()))?;

    let pubkey = match strkey {
        Strkey::PublicKeyEd25519(pk) => pk.0,
        _ => return Err(AppError::BadRequest("Expected G... account address".into())),
    };

    let transaction = build_challenge_envelope(&state, &pubkey)?;

    Ok(Json(ChallengeResponse {
        transaction,
        network_passphrase: state.network_passphrase.clone(),
    }))
}

#[utoipa::path(
    post,
    path = "/auth/verify",
    request_body = VerifyRequest,
    responses(
        (status = 200, description = "JWT token issued", body = VerifyResponse),
        (status = 401, description = "Authentication failed"),
        (status = 503, description = "Verification paused for emergency maintenance")
    ),
    tag = "Auth"
)]
pub async fn verify_handler(
    Extension(state): Extension<Arc<AuthState>>,
    headers: HeaderMap,
    Json(payload): Json<VerifyRequest>,
) -> Result<Json<VerifyResponse>, AppError> {
    if state.is_verification_paused() {
        return Err(AppError::Internal(
            "Message verification is temporarily paused for emergency maintenance".into(),
        ));
    }

    let ip = client_ip_from_headers(&headers).unwrap_or_else(|| "unknown".to_string());
    match verify_challenge_envelope(&state, &payload.transaction) {
        Ok(token) => {
            let subject = decode::<Claims>(
                &token,
                &state.decoding_key,
                &Validation::new(Algorithm::RS256),
            )
            .map(|data| data.claims.sub)
            .unwrap_or_else(|_| "unknown".to_string());
            AuditEvent::new(
                &subject,
                &ip,
                "auth.sep10_verify",
                "/auth/verify",
                AuditOutcome::Success,
            )
            .emit();
            Ok(Json(VerifyResponse { token }))
        }
        Err(err) => {
            AuditEvent::new(
                "anonymous",
                &ip,
                "auth.sep10_verify",
                "/auth/verify",
                AuditOutcome::Failure,
            )
            .with_reason(&err.to_string())
            .emit();
            Err(err)
        }
    }
}

/// Emergency pause toggle endpoint (for administrative control).
/// This endpoint allows operators to pause all message verification in emergency scenarios.
#[utoipa::path(
    post,
    path = "/auth/emergency-pause",
    request_body = EmergencyPauseRequest,
    responses(
        (status = 200, description = "Emergency pause toggled", body = EmergencyPauseResponse),
        (status = 400, description = "Invalid request")
    ),
    tag = "Auth"
)]
pub async fn emergency_pause_handler(
    Extension(state): Extension<Arc<AuthState>>,
    headers: HeaderMap,
    Json(payload): Json<EmergencyPauseRequest>,
) -> Result<Json<EmergencyPauseResponse>, AppError> {
    state.set_verification_paused(payload.paused);
    AuditEvent::new(
        "anonymous",
        &client_ip_from_headers(&headers).unwrap_or_else(|| "unknown".to_string()),
        "admin.emergency_pause",
        "auth/verification",
        AuditOutcome::Success,
    )
    .with_reason(&format!("paused={}", payload.paused))
    .emit();

    let message = if payload.paused {
        "Message verification has been PAUSED for emergency maintenance".to_string()
    } else {
        "Message verification has been RESUMED".to_string()
    };

    Ok(Json(EmergencyPauseResponse {
        paused: payload.paused,
        message,
    }))
}

pub async fn auth_middleware(
    Extension(state): Extension<Arc<AuthState>>,
    req: Request,
    next: Next,
) -> Result<Response, AppError> {
    if state.is_verification_paused() {
        return Err(AppError::Internal(
            "Authentication is temporarily paused for emergency maintenance".into(),
        ));
    }

    // Audit every authentication outcome (Issue #1403).
    let ip = client_ip(&req);
    let resource = req.uri().path().to_string();
    let audit = |actor: &str, outcome: AuditOutcome, reason: &str| {
        AuditEvent::new(actor, &ip, "auth.authenticate", &resource, outcome)
            .with_reason(reason)
            .emit();
    };
    let deny = |reason: String| {
        audit("anonymous", AuditOutcome::Failure, &reason);
        AppError::Unauthorized(reason)
    };

    // Check for API Key first (Issue #32)
    if let Some(api_key) = req.headers().get("X-API-Key").and_then(|v| v.to_str().ok()) {
        // Hash the provided API key to check against stored hashes
        let mut hasher = Sha256::new();
        hasher.update(api_key.as_bytes());
        let hash_hex = hex::encode(hasher.finalize());
        
        let tier = state.api_keys.get(&hash_hex).or_else(|| state.api_keys.get(api_key)).ok_or_else(|| {
            deny("Invalid API Key".into())
        })?;
        // Identify the key by a hash prefix; the raw key is never logged.
        let actor = format!("key_fp:{}", &hash_hex[..12]);

        // Sliding-window rate limiting per API key (Issue #1401)
        let decision = state.rate_limiter.check(
            &format!("key:{hash_hex}"),
            tier.rate_limit_per_minute(),
            Instant::now(),
        );
        if !decision.allowed {
            audit(&actor, AuditOutcome::Failure, "rate limit exceeded");
            return Ok(decision.into_rejection());

        // Constant-time lookup: compare the presented key's digest against
        // every stored key hash without short-circuiting.
        let tier = lookup_api_tier(&state.api_keys, api_key)
            .ok_or_else(|| AppError::Unauthorized("Invalid API Key".into()))?;

        // Rate limiting
        let limit = tier.rate_limit_per_minute();
        // Just use the API key string or hash as the rate limit key
        let cache_key = format!("{}-{}", hash_hex.clone(), now_secs() / 60);
        
        // Simple atomic counter increment using moka
        let count = state.rate_limiter.get_with(cache_key.clone(), || 0);
        if count >= limit {
            return Err(AppError::Unauthorized("Rate limit exceeded".into()));
        }
        audit(&actor, AuditOutcome::Success, "api_key");

        let mut response = next.run(req).await;
        decision.apply_headers(response.headers_mut());
        return Ok(response);
    }

    // Fallback to JWT (Issue #31)
    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| deny("Missing Authorization or X-API-Key header".into()))?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| deny("Expected Bearer token".into()))?;

    let mut validation = Validation::new(Algorithm::RS256);
    validation.validate_exp = true;

    let token_data = decode::<Claims>(token, &state.decoding_key, &validation)
        .map_err(|e| deny(format!("Invalid token: {e}")))?;

    if !token_data.claims.scopes.contains(&"simulate".to_string()) {
        return Err(deny(
            "Missing required scope 'simulate'".into(),
        ));
    }

    // Role validation from claims
    if !token_data.claims.roles.contains(&"admin".to_string()) && !token_data.claims.roles.contains(&"user".to_string()) {
        return Err(deny(
            "Missing required roles".into(),
        ));
    }

    // Requests without an API key fall back to the client IP at the free tier.
    let decision = state.rate_limiter.check(
        &format!("ip:{ip}"),
        ApiTier::Free.rate_limit_per_minute(),
        Instant::now(),
    );
    if !decision.allowed {
        audit(
            &token_data.claims.sub,
            AuditOutcome::Failure,
            "rate limit exceeded",
        );
        return Ok(decision.into_rejection());
    }
    audit(&token_data.claims.sub, AuditOutcome::Success, "jwt");

    let mut response = next.run(req).await;
    decision.apply_headers(response.headers_mut());
    Ok(response)
}

#[derive(Serialize, ToSchema)]
pub struct JwkSetResponse {
    pub keys: Vec<JwkResponse>,
}

#[derive(Serialize, ToSchema)]
pub struct JwkResponse {
    pub kty: String,
    pub alg: String,
    pub kid: String,
    pub n: String,
    pub e: String,
    #[serde(rename = "use")]
    pub use_: String,
}

#[utoipa::path(
    get,
    path = "/auth/jwks",
    responses(
        (status = 200, description = "JSON Web Key Set", body = JwkSetResponse)
    ),
    tag = "Auth"
)]
pub async fn jwks_handler(
    Extension(state): Extension<Arc<AuthState>>,
) -> Result<Json<JwkSetResponse>, AppError> {
    Ok(Json(JwkSetResponse {
        keys: vec![JwkResponse {
            kty: "RSA".to_string(),
            alg: "RS256".to_string(),
            kid: "1".to_string(),
            n: state.jwk_n.clone(),
            e: state.jwk_e.clone(),
            use_: "sig".to_string(),
        }],
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::StatusCode, routing::get, Router};
    use tower::ServiceExt;

    #[test]
    fn sliding_window_counts_down_and_throttles_at_the_limit() {
        let limiter = SlidingWindowLimiter::new(Duration::from_secs(60));
        let start = Instant::now();

        let first = limiter.check("key:a", 3, start);
        assert_eq!(
            first,
            RateLimitDecision {
                allowed: true,
                limit: 3,
                remaining: 2,
                reset_secs: 60,
            }
        );
        assert!(
            limiter
                .check("key:a", 3, start + Duration::from_secs(10))
                .allowed
        );
        let third = limiter.check("key:a", 3, start + Duration::from_secs(20));
        assert!(third.allowed);
        assert_eq!(third.remaining, 0);

        let throttled = limiter.check("key:a", 3, start + Duration::from_secs(30));
        assert!(!throttled.allowed);
        assert_eq!(throttled.remaining, 0);
        // The first request ages out 60s after it was made, i.e. 30s from now.
        assert_eq!(throttled.reset_secs, 30);
    }

    #[test]
    fn sliding_window_frees_slots_as_old_requests_age_out() {
        let limiter = SlidingWindowLimiter::new(Duration::from_secs(60));
        let start = Instant::now();
        assert!(limiter.check("key:a", 2, start).allowed);
        assert!(
            limiter
                .check("key:a", 2, start + Duration::from_secs(30))
                .allowed
        );
        assert!(
            !limiter
                .check("key:a", 2, start + Duration::from_secs(59))
                .allowed
        );

        // Unlike a fixed bucket, only the request from `start` has expired.
        let later = limiter.check("key:a", 2, start + Duration::from_secs(60));
        assert!(later.allowed);
        assert_eq!(later.remaining, 0);
        assert_eq!(later.reset_secs, 30);
    }

    #[test]
    fn sliding_window_tracks_tenants_independently() {
        let limiter = SlidingWindowLimiter::new(Duration::from_secs(60));
        let now = Instant::now();
        assert!(limiter.check("key:a", 1, now).allowed);
        assert!(!limiter.check("key:a", 1, now).allowed);
        assert!(limiter.check("ip:203.0.113.7", 1, now).allowed);
    }

    #[test]
    fn rate_limit_headers_include_retry_after_only_when_throttled() {
        let mut headers = HeaderMap::new();
        RateLimitDecision {
            allowed: true,
            limit: 100,
            remaining: 42,
            reset_secs: 17,
        }
        .apply_headers(&mut headers);
        assert_eq!(headers["x-ratelimit-limit"], "100");
        assert_eq!(headers["x-ratelimit-remaining"], "42");
        assert_eq!(headers["x-ratelimit-reset"], "17");
        assert!(headers.get("retry-after").is_none());

        let rejection = RateLimitDecision {
            allowed: false,
            limit: 100,
            remaining: 0,
            reset_secs: 17,
        }
        .into_rejection();
        assert_eq!(rejection.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(rejection.headers()["retry-after"], "17");
        assert_eq!(rejection.headers()["x-ratelimit-remaining"], "0");
    }

    #[tokio::test]
    async fn auth_middleware_sets_rate_limit_headers_and_returns_429_after_a_burst() {
        let state = Arc::new(AuthState::new(None, Some([7u8; 32]), "Test".into(), false));
        let app = Router::new()
            .route("/protected", get(|| async { "ok" }))
            .route_layer(axum::middleware::from_fn(auth_middleware))
            .layer(Extension(state));

        let request = || {
            axum::http::Request::get("/protected")
                .header("X-API-Key", "hash_free_key")
                .body(Body::empty())
                .unwrap()
        };

        let limit = ApiTier::Free.rate_limit_per_minute();
        for i in 0..limit {
            let response = app.clone().oneshot(request()).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            let headers = response.headers();
            assert_eq!(headers["x-ratelimit-limit"], limit.to_string().as_str());
            assert_eq!(
                headers["x-ratelimit-remaining"],
                (limit - i - 1).to_string().as_str()
            );
            assert!(headers.contains_key("x-ratelimit-reset"));
            assert!(headers.get("retry-after").is_none());
        }

        let throttled = app.clone().oneshot(request()).await.unwrap();
        assert_eq!(throttled.status(), StatusCode::TOO_MANY_REQUESTS);
        let retry_after: u64 = throttled.headers()["retry-after"]
            .to_str()
            .unwrap()
            .parse()
            .unwrap();
        assert!((1..=60).contains(&retry_after));
        assert_eq!(throttled.headers()["x-ratelimit-remaining"], "0");

    #[test]
    fn constant_time_eq_equal_inputs() {
        assert!(constant_time_eq(b"super-secret-api-key", b"super-secret-api-key"));
        assert!(constant_time_eq(b"", b""));
        assert!(constant_time_eq(&[0u8; 64], &[0u8; 64]));
    }

    #[test]
    fn constant_time_eq_different_lengths() {
        assert!(!constant_time_eq(b"short", b"a-much-longer-key"));
        assert!(!constant_time_eq(b"prefix", b"prefix-suffix"));
        assert!(!constant_time_eq(b"", b"non-empty"));
    }

    #[test]
    fn constant_time_eq_single_bit_flip_at_start() {
        let key = b"0123456789abcdef0123456789abcdef";
        let mut flipped = *key;
        flipped[0] ^= 0x01;
        assert!(!constant_time_eq(key, &flipped));
    }

    #[test]
    fn constant_time_eq_single_bit_flip_at_end() {
        let key = b"0123456789abcdef0123456789abcdef";
        let mut flipped = *key;
        let last = flipped.len() - 1;
        flipped[last] ^= 0x01;
        assert!(!constant_time_eq(key, &flipped));
    }

    #[test]
    fn constant_time_eq_single_bit_flip_in_signature() {
        let sig = [0xABu8; 64];
        let mut flipped = sig;
        flipped[0] ^= 0x80;
        assert!(!constant_time_eq(&sig, &flipped));

        let mut flipped_end = sig;
        flipped_end[63] ^= 0x80;
        assert!(!constant_time_eq(&sig, &flipped_end));
    }

    #[test]
    fn lookup_api_tier_matches_stored_hash() {
        let mut api_keys = HashMap::new();
        api_keys.insert("hash_free_key".to_string(), ApiTier::Free);
        api_keys.insert("hash_pro_key".to_string(), ApiTier::Pro);

        assert_eq!(
            lookup_api_tier(&api_keys, "hash_free_key"),
            Some(ApiTier::Free)
        );
        assert_eq!(
            lookup_api_tier(&api_keys, "hash_pro_key"),
            Some(ApiTier::Pro)
        );
        assert_eq!(lookup_api_tier(&api_keys, "unknown_key"), None);
    }
}
