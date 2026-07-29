# Choosing a mode

Start with the default detector. Add one optional feature only when it answers a
question you actually have.

| Mode | Question | Enable | Best place |
| --- | --- | --- | --- |
| Active wait-for detection | Which tracked threads are blocked on one another now? | Default | Tests and production |
| Logging and visualization | How did the execution reach this cycle? | `logging-and-visualization` | Incident capture |
| Lock-order analysis | Have these locks been acquired in a risky order? | `lock-order-graph` | Development and CI |
| Random stress | Can broad timing changes expose the bug? | `stress-test` + `with_random_stress()` | Reproduction tests |
| Component stress | Can targeted delays expose this lock relationship? | `stress-test` + `with_component_stress()` | Focused reproduction |

## Active versus potential

`WaitForGraph` means Deloxide validated a current cycle among tracked waits and
owners.

`LockOrderViolation` means the program previously acquired locks in conflicting
orders. It is useful early warning, but it does not mean threads are blocked
right now.

## A practical progression

1. Reproduce with the default detector.
2. Add logging if the callback IDs are not enough to find the path.
3. Add lock-order analysis in development or CI to catch inversions earlier.
4. Add stress mode only when the failure rarely manifests.
5. Benchmark the exact feature combination before a broad rollout.

Optional features add graph work, event queueing, file I/O, or intentional
delays. They are investigation tools, not a reason to enable everything at once.
