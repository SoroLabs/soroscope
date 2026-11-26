# Oracle Aggregator

The Oracle Aggregator contract is responsible for collecting prices from multiple independent oracle sources and aggregating them into a single, reliable price.

## Median Selection Algorithm

This aggregator uses a median selection algorithm across all submitted oracle feeds. This protects the aggregated price from single outlier price manipulations that would otherwise skew a simple arithmetic mean calculation.

### Outlier Filtering

Once the median is selected, the contract filters out any oracle feeds that deviate too far from the median price, ensuring that malicious or faulty oracles do not impact the system.

## Provider List

Oracle feeds can be registered in an on-chain active list with `register_provider`. The list is bounded by `MAX_ORACLE_PROVIDERS = 32` and rejects duplicate addresses, so sorting and aggregation always fit inside the Soroban CPU instruction budget. `remove_provider` removes decommissioned feeds from the iteration list, and `get_price` aggregates from the currently registered providers. The same cap applies to caller-supplied vectors passed to `aggregate_price`.

## Usage

Provide at least 3 valid oracle addresses to the `aggregate_price` function. The aggregator will query each oracle and return the median price of the valid responses, filtering out outliers.
