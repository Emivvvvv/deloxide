#!/usr/bin/env python3
# scripts/generate_report.py

import json
import os
import csv
from pathlib import Path

def extract_criterion_comparison_data():
    """Extract comparison data from Criterion benchmark results."""
    criterion_dir = Path("/Users/emivvvvv/Documents/GitHub/deloxide/deloxide-tests/target/criterion")

    if not criterion_dir.exists():
        print(f"Error: Criterion directory not found at {criterion_dir}")
        return [], []

    detector_results = []
    detector_log_results = []

    for group_dir in criterion_dir.rglob("*"):
        if not group_dir.is_dir():
            continue

        base_dir   = group_dir / "base"
        new_dir    = group_dir / "new"
        change_dir = group_dir / "change"

        if base_dir.exists() and new_dir.exists() and change_dir.exists():
            try:
                base_file   = base_dir   / "estimates.json"
                new_file    = new_dir    / "estimates.json"
                change_file = change_dir / "estimates.json"

                if not (base_file.exists() and new_file.exists() and change_file.exists()):
                    continue

                base_est   = json.load(open(base_file))
                new_est    = json.load(open(new_file))
                change_est = json.load(open(change_file))

                base_mean = base_est.get("mean", {}).get("point_estimate", None)
                new_mean  = new_est.get("mean",  {}).get("point_estimate", None)
                if base_mean is None or new_mean is None:
                    continue

                change_mean = change_est.get("mean", {}).get("point_estimate", None)
                if change_mean is not None:
                    change_pct = change_mean * 100
                else:
                    change_pct = ((new_mean - base_mean) / base_mean) * 100

                benchmark_name = group_dir.relative_to(criterion_dir).as_posix()

                record = {
                    "benchmark":    benchmark_name,
                    "baseline_ns":  base_mean,
                    "detector_ns":  new_mean,
                    "overhead_pct": change_pct
                }

                if benchmark_name.endswith("_detector_log"):
                    detector_log_results.append(record)
                elif benchmark_name.endswith("_detector"):
                    detector_results.append(record)

            except Exception as e:
                print(f"Warning: Skipping {group_dir}: {e}")

    return detector_results, detector_log_results


def write_report(file_name, results):
    os.makedirs("results", exist_ok=True)
    with open(file_name, "w", newline="") as f:
        fieldnames = ["benchmark", "baseline_ns", "detector_ns", "overhead_pct"]
        writer = csv.DictWriter(f, fieldnames=fieldnames)
        writer.writeheader()
        writer.writerows(results)


def print_summary(title, results):
    print(f"\n{title}")
    print("-" * 60)
    print(f"{'Benchmark':<40} {'Overhead %':>15}")
    print("-" * 60)

    for result in sorted(results, key=lambda x: x['overhead_pct']):
        print(f"{result['benchmark']:<40} {result['overhead_pct']:>14.2f}%")

    print("-" * 60)

    overheads = [r["overhead_pct"] for r in results]
    if overheads:
        avg = sum(overheads) / len(overheads)
        print(f"{'Average overhead:':40} {avg:>14.2f}%")
        print(f"{'Minimum overhead:':40} {min(overheads):>14.2f}%")
        print(f"{'Maximum overhead:':40} {max(overheads):>14.2f}%")

        by_type = {}
        for result in results:
            bench_type = result['benchmark'].split('/')[0] if '/' in result['benchmark'] else result['benchmark']
            if bench_type not in by_type:
                by_type[bench_type] = []
            by_type[bench_type].append(result['overhead_pct'])

        print("\nOverhead by benchmark type:")
        print("-" * 60)
        for bench_type, overheads in sorted(by_type.items()):
            avg = sum(overheads) / len(overheads)
            print(f"{bench_type:<40} {avg:>14.2f}%")


def main():
    detector_results, detector_log_results = extract_criterion_comparison_data()

    if not detector_results and not detector_log_results:
        print("No comparison results available. Make sure to run all baseline and detector benchmarks.")
        return

    # Save results
    write_report("results/overhead_detector.csv", detector_results)
    write_report("results/overhead_detector_log.csv", detector_log_results)

    # Print summaries
    if detector_results:
        print_summary("Performance Overhead: Baseline → Detector (No Log)", detector_results)
    if detector_log_results:
        print_summary("Performance Overhead: Baseline → Detector + Log", detector_log_results)

    print("\nReports written to:")
    print(" - results/overhead_detector.csv")
    print(" - results/overhead_detector_log.csv")


if __name__ == "__main__":
    main()