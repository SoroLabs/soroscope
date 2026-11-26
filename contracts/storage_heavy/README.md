# Storage-heavy contract

This contract provides storage-cost benchmarks for persistent and temporary
Soroban entries, including batch access and compact boolean representations.

## Keeping persistent entries alive

Any account can extend a persistent entry's lifetime by invoking:

```text
extend_ttl(key, threshold, extend_to)
```

The entry is extended to `extend_to` ledgers only when its remaining lifetime
is below `threshold`. The function requires no authorization so the invoker can
pay the network rent on behalf of the entry owner. The key must already exist
in persistent storage; temporary entries are intentionally not affected.
