# Asset provenance

- `mutex-latency.svg` is generated from
  `docs/performance/results/2026-07-29-fast-path.csv` with
  `python3 scripts/generate_doc_charts.py`.
- `manifestation-rate.svg` is generated from
  `docs/performance/results/2026-07-29-manifestation.csv` with the same command.
  The committed chart explicitly states when the prescribed manifestation run did
  not produce comparable data; it does not represent a rate in that case.
- `visualization.png` is the migrated product screenshot from
  `images/visualization.png`. It is not benchmark evidence.

Check generated assets without writing files with:

```sh
python3 scripts/generate_doc_charts.py --check
```
