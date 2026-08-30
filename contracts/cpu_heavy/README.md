# CPU Heavy Contract

A benchmark contract whose entry points deliberately burn CPU, used to observe
how SoroScope reports invocation cost. Every entry point is a pure computation:
there is no state, no authorisation, and no storage access.

## Hard caps

Each entry point checks its arguments against a hard cap *before* doing any
work. Without that check an oversized argument runs until the host aborts the
invocation with `Error(Budget, ExceededLimit)`, which reaches the caller as an
opaque `UnreachableCodeReached` VM trap that names neither the offending
argument nor the limit it broke. The guards turn that into a contract error
code instead.

| Constant             | Value     | Applies to                            |
| -------------------- | --------- | ------------------------------------- |
| `MAX_FIB`            | `50_000`  | `fibonacci_iterative(n)`              |
| `MAX_SORT`           | `100`     | `bubble_sort(values)` length          |
| `MAX_PRIME`          | `20_000`  | `count_primes(limit)`                 |
| `MAX_LOOP_OPS`       | `500_000` | `nested_loop_burn(outer * inner)`     |
| `MAX_COMBINED_FIB`   | `10_000`  | `combined_benchmark(fib_n, ..)`       |
| `MAX_COMBINED_SORT`  | `50`      | `combined_benchmark(.., sort_size, ..)` |
| `MAX_COMBINED_PRIME` | `5_000`   | `combined_benchmark(.., prime_limit)` |

The caps are chosen so the most expensive legal call still fits in the default
network budget of 100,000,000 CPU instructions and 41,943,040 memory bytes.
Measured against the release WASM build with every argument at its cap:

| Call                                    | CPU        | Memory  |
| --------------------------------------- | ---------- | ------- |
| `fibonacci_iterative(50_000)`           | 1.6M (1%)  | 3%      |
| `count_primes(20_000)`                  | 22.7M (22%)| 3%      |
| `bubble_sort([100 elements])`           | 60.1M (60%)| 24%     |
| `nested_loop_burn(500_000, 1)`          | 0.8M (0%)  | 3%      |
| `combined_benchmark(10_000, 50, 5_000)` | 19.2M (19%)| 6%      |

`bubble_sort` is the binding constraint: it is quadratic in *host* calls
(`Vec::get`/`Vec::set`), so its cost grows far faster than the other workloads.
At 150 elements it costs 136.8M CPU and exceeds the budget outright, which is
why the cap sits at 100.

`nested_loop_burn` guards on the `outer * inner` product using
`saturating_mul`, so a caller cannot overflow the multiplication to wrap past
the cap.

## API

`fibonacci_iterative(n: u32) -> Result<u64, Error>`

Iterative Fibonacci, wrapping on `u64` overflow. Returns
`FibonacciInputTooLarge` if `n > MAX_FIB`.

`bubble_sort(values: Vec<u32>) -> Result<Vec<u32>, Error>`

Bubble-sorts the list in place. Returns `SortInputTooLarge` if the list is
longer than `MAX_SORT`.

`count_primes(limit: u32) -> Result<u32, Error>`

Counts primes in `2..=limit` by trial division. Returns `PrimeLimitTooLarge` if
`limit > MAX_PRIME`.

`nested_loop_burn(outer: u32, inner: u32) -> Result<u64, Error>`

Burns `outer * inner` iterations of trivial arithmetic. Returns
`LoopOpsTooLarge` if the product exceeds `MAX_LOOP_OPS`.

`combined_benchmark(fib_n: u32, sort_size: u32, prime_limit: u32) -> Result<Vec<u64>, Error>`

Runs all three workloads in one invocation and returns
`[fibonacci, prime_count]`. Returns `CombinedInputTooLarge` if any argument
exceeds its sub-cap.

## Errors

| Code | Variant                  |
| ---- | ------------------------ |
| 1    | `FibonacciInputTooLarge` |
| 2    | `SortInputTooLarge`      |
| 3    | `PrimeLimitTooLarge`     |
| 4    | `LoopOpsTooLarge`        |
| 5    | `CombinedInputTooLarge`  |

The discriminants are part of the contract's public surface — the SoroScope UI
decodes errors by number — so new variants must be appended rather than
inserted.

## Tests

```sh
cargo test -p cpu_heavy
```
