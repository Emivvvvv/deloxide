# Running in Production

Begin with the default active detector and a bounded callback. Enable event logging
only when its diagnostic value justifies the optional queue and file I/O. Monitor
callback failures and preserve the reported lock/thread mapping with application
request identifiers.

Deloxide observes only its own wrappers. Mixing a tracked lock path with raw or
third-party synchronization can hide dependencies. Establish a rollout benchmark
with the same feature set and workload used in production.
