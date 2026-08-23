#!/usr/bin/env python3
"""Aggregate repeated sweep CSVs from one commit by pointwise median.

Each input must use the normal sweep CSV schema and have a sibling meta.json
with the same git_sha. The output intentionally retains that schema, so it can
be passed directly to cv-compare.py and the existing plot scripts.

Usage:
    scripts/aggregate-sweeps.py output.csv run-a/sweep.csv run-b/sweep.csv
"""

import argparse
import csv
import json
from collections import defaultdict
from pathlib import Path

FIELDNAMES = ["operation", "design", "n", "ns_per_op"]


def load(path: Path):
    meta_path = path.parent / "meta.json"
    with meta_path.open() as meta_file:
        sha = json.load(meta_file)["git_sha"]

    rows = []
    with path.open(newline="") as csv_file:
        reader = csv.DictReader(csv_file)
        if reader.fieldnames != FIELDNAMES:
            raise ValueError(f"{path}: expected CSV columns {FIELDNAMES}")
        for row in reader:
            rows.append((row["operation"], row["design"], int(row["n"]), float(row["ns_per_op"])))
    return sha, rows


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    parser.add_argument("inputs", nargs="+", type=Path)
    args = parser.parse_args()

    by_point = defaultdict(list)
    shas = set()
    for path in args.inputs:
        sha, rows = load(path)
        shas.add(sha)
        for operation, design, n, ns_per_op in rows:
            by_point[(operation, design, n)].append(ns_per_op)

    if len(shas) != 1:
        raise ValueError(f"refusing to aggregate different git SHAs: {', '.join(sorted(shas))}")

    with args.output.open("w", newline="") as output_file:
        writer = csv.writer(output_file)
        writer.writerow(FIELDNAMES)
        for operation, design, n in sorted(by_point):
            samples = sorted(by_point[(operation, design, n)])
            # Match the sweep harness's deterministic upper median for even samples.
            writer.writerow((operation, design, n, f"{samples[len(samples) // 2]:.2f}"))

    print(
        f"aggregated {len(args.inputs)} CSVs at {next(iter(shas))}: "
        f"{len(by_point)} points -> {args.output}"
    )


if __name__ == "__main__":
    main()
