use crate::errors::AppError;
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

const CHALLENGE_EXPIRY_SECS: u64 = 300;
const JWT_EXPIRY_SECS: u64 = 86400;
const WEB_AUTH_DOMAIN: &str = "soroscope";

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
}

impl AuthState {
    pub fn new(
        jwt_private_key_pem: Option<String>,
        sep10_seed: Option<[u8; 32]>,
        network_passphrase: String,
        emergency_verification_paused: bool,
    ) -> Self {
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
        self.emergency_verification_paused.store(paused, Ordering::SeqCst);
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
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
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

    let hint: [u8; 4] = state.server_public_key[28..32].try_into().unwrap();
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
    let server_hint: [u8; 4] = state.server_public_key[28..32].try_into().unwrap();
    let client_hint: [u8; 4] = client_key[28..32].try_into().unwrap();

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
    Json(payload): Json<VerifyRequest>,
) -> Result<Json<VerifyResponse>, AppError> {
    if state.is_verification_paused() {
        return Err(AppError::Internal(
            "Message verification is temporarily paused for emergency maintenance".into(),
        ));
    }

    let token = verify_challenge_envelope(&state, &payload.transaction)?;
    Ok(Json(VerifyResponse { token }))
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
    Json(payload): Json<EmergencyPauseRequest>,
) -> Result<Json<EmergencyPauseResponse>, AppError> {
    state.set_verification_paused(payload.paused);

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
    // Check if verification is paused — deny all requests during emergency maintenance
    if state.is_verification_paused() {
        return Err(AppError::Internal(
            "Authentication is temporarily paused for emergency maintenance".into(),
        ));
    }

    let auth_header = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| AppError::Unauthorized("Missing Authorization header".into()))?;

    let token = auth_header
        .strip_prefix("Bearer ")
        .ok_or_else(|| AppError::Unauthorized("Expected Bearer token".into()))?;

    let validation = Validation::new(Algorithm::RS256);
    let token_data = decode::<Claims>(token, &state.decoding_key, &validation)
        .map_err(|e| AppError::Unauthorized(format!("Invalid token: {e}")))?;

    if !token_data.claims.scopes.contains(&"simulate".to_string()) {
        return Err(AppError::Unauthorized(
            "Missing required scope 'simulate'".into(),
        ));
    }

    Ok(next.run(req).await)
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
