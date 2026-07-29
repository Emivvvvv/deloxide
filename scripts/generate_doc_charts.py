#!/usr/bin/env python3
"""Generate deterministic SVG charts from recorded documentation evidence."""

import argparse
import csv
import sys
from html import escape
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
CHARTS = {
    ROOT / "docs/performance/results/2026-07-29-fast-path.csv": (
        ROOT / "docs/src/assets/mutex-latency.svg",
        "latency",
    ),
    ROOT / "docs/performance/results/2026-07-29-manifestation.csv": (
        ROOT / "docs/src/assets/manifestation-rate.svg",
        "manifestation",
    ),
}


def read_rows(path: Path) -> list[dict[str, str]]:
    with path.open(newline="", encoding="utf-8") as handle:
        return list(csv.DictReader(handle))


def bar_chart(
    title: str,
    description: str,
    values: list[tuple[str, float, str]],
) -> str:
    width = 960
    row_height = 42
    height = 100 + row_height * len(values)
    maximum = max(value for _, value, _ in values)
    plot_width = 560
    rows = []
    for index, (label, value, display) in enumerate(values):
        y = 70 + index * row_height
        bar_width = 0 if maximum == 0 else value / maximum * plot_width
        rows.append(
            f'<text x="20" y="{y + 18}">{escape(label)}</text>'
            f'<rect x="310" y="{y}" width="{bar_width:.2f}" height="24" '
            'fill="#e66a2c"/>'
            f'<text x="{320 + bar_width:.2f}" y="{y + 18}">'
            f'{escape(display)}</text>'
        )
    return (
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" '
        f'height="{height}" viewBox="0 0 {width} {height}">'
        f"<title>{escape(title)}</title>"
        f"<desc>{escape(description)}</desc>"
        '<style>text{font:14px sans-serif;fill:#222}</style>'
        + "".join(rows)
        + "</svg>\n"
    )


def no_data_chart(title: str, description: str) -> str:
    return (
        '<svg xmlns="http://www.w3.org/2000/svg" width="960" height="142" '
        'viewBox="0 0 960 142">'
        f"<title>{escape(title)}</title>"
        f"<desc>{escape(description)}</desc>"
        '<style>text{font:14px sans-serif;fill:#222}</style>'
        '<text x="20" y="42">No comparable result was recorded.</text>'
        '<text x="20" y="72">See the evaluation record for the runner limitation.</text>'
        '</svg>\n'
    )


def latency_svg(rows: list[dict[str, str]]) -> str:
    selected = [
        row
        for row in rows
        if row["case"] != "mutex/two_thread_handoff" and row["unit"] == "ns"
    ]
    values = [
        (
            f'{row["implementation"]}: {row["case"]}',
            float(row["median"]),
            f'{float(row["median"]):.2f} ns',
        )
        for row in selected
    ]
    return bar_chart(
        "Focused synchronization latency",
        "Median nanoseconds per uncontended operation; lower is better.",
        values,
    )


def manifestation_svg(rows: list[dict[str, str]]) -> str:
    if not rows:
        return no_data_chart(
            "Deadlock manifestation rate",
            "No comparable 1,000-iteration result was recorded because the "
            "evaluation runner did not complete in reasonable time.",
        )
    values = [
        (
            f'{row["scenario"]}: {row["mode"]}',
            float(row["rate_percent"]),
            f'{float(row["rate_percent"]):.1f}%',
        )
        for row in rows
    ]
    return bar_chart(
        "Deadlock manifestation rate",
        "Share of runs producing an active wait-for cycle; higher means the "
        "schedule exposed the intended deadlock more often.",
        values,
    )


def render(kind: str, rows: list[dict[str, str]]) -> str:
    if kind == "latency":
        return latency_svg(rows)
    if kind == "manifestation":
        return manifestation_svg(rows)
    raise ValueError(f"unknown chart kind: {kind}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()

    stale = []
    for input_path, (output_path, kind) in CHARTS.items():
        content = render(kind, read_rows(input_path))
        if args.check:
            if not output_path.exists() or output_path.read_text(encoding="utf-8") != content:
                stale.append(output_path)
        else:
            output_path.write_text(content, encoding="utf-8")
    for output_path in stale:
        print(output_path)
    return 1 if stale else 0


if __name__ == "__main__":
    sys.exit(main())
