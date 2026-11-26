# Contract event webhooks

`soroscope-core::webhooks` provides the outbound delivery layer for contract
subscriptions. Event ingestion creates a `ContractEvent` and enqueues it through
`WebhookSender`; the worker finds active subscriptions for that contract and
event type without blocking the ingestion task.

Each request contains:

- `x-soroscope-delivery`: a stable UUID for all attempts of one delivery
- `x-soroscope-timestamp`: the Unix timestamp used for the attempt
- `x-soroscope-signature`: `sha256=<hex HMAC-SHA256>`

The signed bytes are `<timestamp>.<raw request body>`. Consumers should reject
old timestamps, compute the HMAC over the unmodified body, and compare it in
constant time. The Rust `webhooks::verify` helper implements the signature
comparison.

```rust
let registry = SubscriptionRegistry::default();
registry.insert(ContractSubscription::new(
    contract_id,
    vec!["transfer".into()],
    callback_url,
    signing_secret,
)?).await;

let worker = WebhookWorker::start(registry, WebhookConfig::default());
worker.sender.enqueue(contract_event).await?;
```

Network failures, HTTP 408, HTTP 429, and 5xx responses are retried with
exponential backoff capped by `retry_max`. Other 4xx responses fail immediately.
Queue capacity, request timeout, attempt count, and retry bounds are configurable.
