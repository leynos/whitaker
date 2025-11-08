# Whitaker

This is a generated project using [Copier](https://copier.readthedocs.io/).

## Testing

Whenever a cluster-only outage occurs the fixture panics with a
`SKIP-TEST-CLUSTER` prefix, so higher-level harnesses can treat the failure as
a soft skip rather than a regression.
