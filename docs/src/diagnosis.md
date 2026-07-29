# Diagnosis and Visualization

Start with `thread_cycle`, then map each thread through
`thread_waiting_for_locks`. Confirm which guard remains live at each acquisition
site. For lock-order reports, inspect `lock_order_cycle` and reproduce with stress
mode before treating the pattern as an active incident.

With `logging-and-visualization`, configure a log path and open it with the showcase
API. Logging is optional and has different memory/I/O characteristics from the
default detector.
