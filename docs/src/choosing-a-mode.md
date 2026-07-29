# Choosing a Mode

| Capability | Finds | Best use |
|---|---|---|
| Active wait-for graph | Cycles among currently blocked tracked threads | Runtime diagnosis |
| Lock-order graph | Inconsistent historical acquisition orders | Development and CI |
| Stress mode | Schedules more likely to manifest a bug | Reproduction tests |
| Logging and visualization | An event history and interactive explanation | Incident analysis |

Start with the default active detector. Enable lock-order analysis when a dangerous
order may not manifest during the test. Enable stress only in controlled testing.
Enable logging when the event history is worth its optional I/O and memory cost.
