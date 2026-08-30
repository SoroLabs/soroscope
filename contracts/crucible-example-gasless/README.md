# Crucible Gasless Vault

A meta-transaction (gasless) contract. A user signs a `MetaTx` off-chain; a
trusted relayer submits it on-chain and pays the network fee; the contract
verifies the request and performs the token transfer on the user's behalf.

In Soroban "signing" is `require_auth` — the relayer attaches the user's
authorization entry to the transaction. The contract enforces relayer-only
submission, a deadline, and single-use nonces.

## Nonce model

Each account has its own monotonically increasing counter holding the **next
expected nonce**. A meta-transaction is valid only if its `nonce` equals that
counter exactly; executing it advances the counter by one, which retires that
nonce permanently. Meta-transactions for an account are therefore consumed
strictly in order, and every nonce is usable at most once.

`nonce(user)` returns the next expected value. A user who has never transacted
reads `0` and has no ledger entry at all.

### Storage

Nonces live in **persistent** storage under `DataKey::Nonce(Address)` — one
ledger entry per account. Both parts of that matter for replay protection:

- **Not temporary storage.** A temporary entry is deleted when it expires, and
  a deleted entry reads back as `0`. That would rewind the counter and make
  every already-consumed nonce replayable. A persistent entry is archived
  rather than deleted; archived state must be restored, value intact, before
  the contract can be invoked against it, so the counter cannot silently
  rewind.
- **Not instance storage.** Instance storage is a single ledger entry shared by
  the whole contract. Packing every account's nonce into it means reading and
  rewriting all of them on every execution, and the contract stops working once
  enough accounts have been seen to exceed the entry size limit.

Every write refreshes the entry's TTL (`NONCE_TTL_THRESHOLD` /
`NONCE_TTL_EXTEND_TO`, roughly 30 and 60 days at a 5s close time), so an
account stays live as long as it keeps transacting.

> [!WARNING]
> Nonces previously lived in instance storage. This is a breaking storage
> layout change: an already-deployed instance must be redeployed or migrated,
> because existing counters will not be found at the new keys and every account
> would read back as `0`.

## Batch invalidation

`invalidate_nonces(user, new_nonce) -> u64`

Advances `user`'s counter to `new_nonce`, retiring the whole contiguous range
`[current, new_nonce)` at once. This is how a user cancels meta-transactions
they have already signed but that have not been submitted — for example after
handing a batch to a relayer that never broadcast them.

Because invalidation moves the same counter that `execute` checks, a cancelled
meta-tx fails the ordinary nonce check; there is no separate revocation list to
consult. It is a single O(1) ledger write, so cancelling a hundred pending
meta-transactions costs exactly as much as cancelling one.

Requires `user`'s authorization, so a relayer can submit it on the user's behalf
exactly like an `execute` — cancelling is gasless too.

Constraints:

- `new_nonce` must be strictly greater than the current counter. Rewinding
  would resurrect dead nonces, and a no-op is treated as a caller mistake
  rather than a silent success. Both revert with `invalid nonce`.
- A single call may not advance the counter by more than `MAX_NONCE_ADVANCE`
  (10,000), reverting with `advance too large`. The cap is not about cost —
  the write is O(1) either way — it stops one mistaken call from pushing the
  counter so far that the account can never issue a usable meta-tx again. The
  limit applies to the *jump*, not the absolute value, so repeated calls can
  advance the counter arbitrarily far.

## API

`initialize(admin)`

Records the trusted relayer. Callable once; requires `admin` auth.

`execute(relayer, meta_tx)`

Executes a meta-transaction. Requires the registered relayer's auth *and*
`meta_tx.from`'s auth. Reverts on an unregistered relayer
(`unauthorized relayer`), a past deadline (`meta-tx expired`), or a nonce that
does not match the counter (`invalid nonce`). Emits `executed`.

The nonce is consumed before the token transfer, following
checks-effects-interactions. Soroban forbids contract re-entry at the host
level, so a hostile token cannot call back into `execute` regardless; the
ordering means replay protection does not depend on that guarantee.

`invalidate_nonces(user, new_nonce) -> u64`

See above. Emits `invalidated` with the old and new counter values.

`nonce(user) -> u64`

The next expected nonce for `user`.

`relayer() -> Address`

The registered relayer.

## Tests

```sh
cargo test -p crucible-example-gasless
```

The suite covers execution, replay, deadlines, relayer authorization, per-
account nonce independence, storage placement, batch invalidation and its
bounds, and re-entrancy.

Note that most tests run under `mock_all_auths()`, which makes any
`require_auth` succeed. Two tests deliberately do not, and construct explicit
authorization trees instead, so that a missing `require_auth` in the contract
cannot pass unnoticed:

- `test_execute_without_user_auth_reverts` — the relayer authorizes but the
  user does not, and execution fails.
- `test_execute_succeeds_when_both_parties_authorize` — the same call with the
  user's authorization added succeeds, proving the test above fails for the
  intended reason rather than because of its narrower mock setup.
