# Asset provenance

- `mutex-latency.svg` is generated from
  `docs/performance/results/2026-07-29-fast-path.csv` with
  `python3 scripts/generate_doc_charts.py`.
- `manifestation-rate.svg` is generated from
  `docs/performance/results/2026-07-29-manifestation.csv` with the same command.
  It contains the 20 populated scenario/mode rates from the completed
  1,000-run-per-combination measurement.
- Generated SVG charts include an explicit white background and dark text so
  their labels remain readable in both light and dark book themes.
- `visualization.png` is the migrated product screenshot from
  `images/visualization.png`. It is not benchmark evidence.

Check generated assets without writing files with:

```sh
python3 scripts/generate_doc_charts.py --check
```
