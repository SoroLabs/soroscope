# Staking Rewards Contract

The staking rewards contract lets users stake a principal token, accrue reward-token payouts using an epoch-based compounding schedule with decay, and recover principal through normal or emergency withdrawal paths.

## Epoch Snapshot System

Rewards accrue at a rate that is constant within each epoch and decays at epoch boundaries.

- **epoch_length**: blocks per epoch (configurable at initialization)
- **epoch_decay_percent**: percentage reduction in emission rate at each epoch boundary
- Within an epoch: `rate = initial_rate * (1 - epoch_decay_percent)^epoch`
- Snapshots are created lazily on first access and stored on-chain for transparency

This replaces the previous continuous per-block decay model, providing predictable per-epoch rates and reduced computational overhead.

## Initialization

`initialize(owner, staking_token, reward_token, initial_rate, epoch_decay_percent, epoch_length, start_block) -> Result<(), ContractError>`

Creates the contract configuration once. `initial_rate` and `epoch_decay_percent` use the contract's 18-decimal fixed-point scale. `epoch_decay_percent` must be between `0` and `SCALE`, `epoch_length` must be non-zero, and `initial_rate` must be non-negative. The epoch 0 snapshot is created during initialization.

## Mutating API

`stake(user, amount) -> Result<(), ContractError>`

Transfers `amount` of the staking token from `user` into the contract, updates accrued rewards, and increases the user's staked balance. Requires `user` auth. Fails while paused or when `amount <= 0`.

`withdraw(user, amount) -> Result<(), ContractError>`

Updates accrued rewards, decreases the user's staked balance, and transfers `amount` of the staking token back to `user`. Requires `user` auth. Fails while paused, when `amount <= 0`, or when the user has insufficient staked balance.

`claim(user) -> Result<i128, ContractError>`

Updates accrued rewards, resets claimable rewards to zero, and transfers reward tokens to `user`. Requires `user` auth. Returns `0` when no rewards are available. Fails while paused.

`emergency_withdraw(user) -> Result<i128, ContractError>`

Transfers all staked principal back to `user` and deletes the user's staking state, forfeiting any accrued rewards. Requires `user` auth. This path is available while paused.

`set_paused(paused) -> Result<(), ContractError>`

Owner-only circuit breaker. When paused, `stake`, `withdraw`, and `claim` fail with `Paused`; `emergency_withdraw` remains available.

## Read API

`get_staked_balance(user) -> i128`

Returns the user's staked principal balance, or `0` when no state exists.

`get_accrued_rewards(user) -> i128`

Returns rewards persisted during the user's last state update, or `0` when no state exists.

`get_pending_rewards(user) -> i128`

Returns accrued rewards plus rewards accumulated since the last update, using the current ledger sequence and epoch-based decay.

`get_config() -> Result<StakingConfig, ContractError>`

Returns the full staking configuration.

## Events

`stake`: publishes `StakeEvent { user, amount }`.

`withdraw`: publishes `WithdrawEvent { user, amount }`.

`claim`: publishes `ClaimEvent { user, amount }`.

`emergency_withdraw`: publishes `EmergencyWithdrawEvent { user, amount }`.

`set_paused`: publishes `PausedEvent { paused }`.

## Error Codes

The contract uses `soroscope_error_codes::ContractError`.

| Code | Variant | Meaning |
| --- | --- | --- |
| 1 | `AlreadyInitialized` | `initialize` was called after configuration already exists. |
| 2 | `NotInitialized` | Configuration is missing. |
| 3 | `Unauthorized` | Caller is not authorized for the requested action. |
| 4 | `InsufficientBalance` | User does not have enough staked principal to withdraw. |
| 5 | `InsufficientLiquidity` | Shared error code; not currently emitted by this contract. |
| 6 | `InsufficientShares` | Shared error code; not currently emitted by this contract. |
| 7 | `InsufficientAllowance` | Shared error code; not currently emitted by this contract. |
| 8 | `SlippageExceeded` | Shared error code; not currently emitted by this contract. |
| 9 | `InvalidFee` | Shared error code; not currently emitted by this contract. |
| 10 | `NoPendingFeeUpdate` | Shared error code; not currently emitted by this contract. |
| 11 | `TimelockNotElapsed` | Shared error code; not currently emitted by this contract. |
| 12 | `OracleNotConfigured` | Shared error code; not currently emitted by this contract. |
| 13 | `InvalidOraclePrice` | Shared error code; not currently emitted by this contract. |
| 14 | `Paused` | State-changing reward operations are paused. |
| 15 | `Overflow` | Fixed-point math or integer arithmetic overflowed. |
| 16 | `DivisionByZero` | Fixed-point division attempted to divide by zero. |
| 17 | `InvalidInput` | Input amount or rate configuration is invalid. |
