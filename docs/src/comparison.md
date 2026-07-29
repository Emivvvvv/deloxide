# Why Deloxide

The Rust ecosystem offers several approaches to concurrency safety, each with
different trade-offs. Deloxide is built to bridge the gap between lightweight
but passive monitoring and heavyweight synchronous debugging.

## The landscape

**Static analysis** checks code before it runs. It can find useful ordering
problems early, but complex path and concurrency assumptions can produce noisy
results. It also cannot reconstruct the runtime schedule that produced an
incident.

**Passive dynamic detection**, such as a periodic `parking_lot` deadlock check,
keeps normal lock operations fast. Because observation happens later and the
detector does not perturb scheduling, timing-sensitive bugs may fail to manifest
or may only be reported at the next polling interval.

**Synchronous graph analysis**, represented in the evaluation by
`no_deadlocks`, updates a global model around lock operations and finds cycles
immediately. The full evaluation shows why that approach is normally
treated as a debugging configuration rather than an always-on production path.

**Deloxide** combines synchronous active detection with an Optimistic Fast Path.
Eligible uncontended Mutex and exclusive RwLock operations avoid global graph
work, while contended operations publish the evidence needed for a current
wait-for cycle. Optional features add predictive lock-order analysis, schedule
stress, logging, and visualization only when the investigation needs them.

## Feature matrix

| Feature | STD | PL+DD | ND | **DX** |
| :--- | :---: | :---: | :---: | :---: |
| **Mutex overhead** | 0.88× | 1.00× | 1063.33× | **1.09×** |
| **Raytracing at 1080p** | 0.94× | 1.00× | 17.96× | **0.91× (faster)** |
| **Detection method** | None | Async (poll) | Synchronous | **Synchronous (instant)** |
| **Lock-order analysis** | No | No | No | **Yes** |
| **Stress testing** | No | No | No | **Yes** |
| **Visualization** | No | No | Text dump | **Interactive URL** |
| **False-positive rate in evaluated WFG controls** | N/A | Zero | Zero | **Zero** |

*STD = `std::sync`, PL+DD = `parking_lot` with `deadlock_detection`, ND =
`no_deadlocks`, DX = Deloxide. Ratios and observed false-positive results are from
the full evaluation.*

## What Deloxide adds

Deloxide covers the full lifecycle of a concurrency defect:

- **Development:** lock-order analysis finds dangerous inversions before they
  block a run.
- **Testing:** random and component-based stress modes make rare schedules
  substantially easier to manifest.
- **Diagnosis:** active WFG reports identify the participating threads and waited
  locks immediately.
- **Response:** custom callbacks can record evidence, send alerts, export
  telemetry, or notify an application supervisor.
- **Investigation:** structured logs become an interactive execution timeline and
  dependency graph.
- **Production:** the Optimistic Fast Path keeps the default detector close to
  primitive-baseline cost in the evaluated workloads.
- **Integration:** Rust applications get guard-based wrappers and C applications
  use the same detector through the shipped header.

That combination is Deloxide's selling point. It is not only another lock
implementation and not only a post-hoc deadlock check; it is one toolkit for
finding, reproducing, explaining, and monitoring the bug.

The detailed methodology and results are in
[Performance and benchmarks](production/performance.md) and the
[Deloxide preprint](https://papers.ssrn.com/sol3/papers.cfm?abstract_id=6389109).
