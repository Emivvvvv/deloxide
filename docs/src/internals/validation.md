# Validation and False-Positive Controls

All edges in an active cycle are checked against current mode-aware ownership before
dispatch. Shared read ownership cannot suppress a cycle. Lock-order findings remain
separately labeled as potential rather than active deadlocks.
