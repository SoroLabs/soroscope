# Oracle Aggregator

The Oracle Aggregator contract is responsible for collecting prices from multiple independent oracle sources and aggregating them into a single, reliable price.

## Median Selection Algorithm

This aggregator uses a median selection algorithm across all submitted oracle feeds. This protects the aggregated price from single outlier price manipulations that would otherwise skew a simple arithmetic mean calculation.

### Outlier Filtering

Once the median is selected, the contract filters out any oracle feeds that deviate too far from the median price, ensuring that malicious or faulty oracles do not impact the system.

## Usage

Provide at least 3 valid oracle addresses to the `aggregate_price` function. The aggregator will query each oracle and return the median price of the valid responses, filtering out outliers.
