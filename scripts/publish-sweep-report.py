#!/usr/bin/env python3
"""Publish a reproducible Markdown/SVG report from repeated sweep runs.

Inputs are run directories containing sweep.csv, meta.json, and (optionally)
sweep.args. Runs must be comparable: identical SHA, host, governor, command,
and CSV point grid. The report shows the median curve plus the full range of
per-run point medians; this is descriptive uncertainty, not a confidence
interval or a CV claim.

Usage:
    scripts/publish-sweep-report.py docs/src/benchmarks/reports/run.md run-a run-b
"""

import argparse
import csv
import json
import math
import shutil
from collections import defaultdict
from pathlib import Path
from statistics import median

FIELDS = ["operation", "design", "n", "ns_per_op"]
WIDTH, HEIGHT = 1000, 560
PAD = (72, 26, 38, 76)  # left, right, top, bottom


def fail(message):
    raise ValueError(message)


def metadata(run):
    path = run / "meta.json"
    with path.open() as source:
        raw = json.load(source)
    sha = raw.get("git_sha", raw.get("commit"))
    host = raw.get("host", raw.get("machine"))
    governor = raw.get("governor", raw.get("cpu_governor"))
    date = raw.get("started", raw.get("created"))
    command = raw.get("command", raw.get("bench"))
    args = run / "sweep.args"
    if args.exists():
        command = "cargo bench --bench sweep -- " + args.read_text().strip()
    required = {"SHA": sha, "date": date, "host": host, "governor": governor, "command": command}
    missing = [name for name, value in required.items() if not value]
    if missing:
        fail(f"{path}: missing {', '.join(missing)}")
    return {"sha": str(sha), "date": str(date), "host": str(host), "governor": str(governor), "command": str(command), "load": str(raw.get("loadavg_start", "not recorded"))}


def load_run(run):
    info = metadata(run)
    csv_path = run / "sweep.csv"
    with csv_path.open(newline="") as source:
        reader = csv.DictReader(source)
        if reader.fieldnames != FIELDS:
            fail(f"{csv_path}: expected CSV columns {FIELDS}")
        points = {}
        for row in reader:
            key = (row["operation"], row["design"], int(row["n"]))
            if key in points:
                fail(f"{csv_path}: duplicate point {key}")
            points[key] = float(row["ns_per_op"])
    if not points:
        fail(f"{csv_path}: no data rows")
    return info, points


def svg_escape(value):
    return str(value).replace("&", "&amp;").replace("<", "&lt;").replace(">", "&gt;")


def color(index):
    palette = ["#b2182b", "#2166ac", "#1b7837", "#e08214", "#7b3294", "#00838f", "#d73027", "#525252", "#542788", "#4d9221", "#c51b7d", "#636363"]
    return palette[index % len(palette)]


def write_svg(path, operation, data):
    designs = sorted({design for op, design, _ in data if op == operation})
    ns = sorted({n for op, _, n in data if op == operation})
    values = [value for (op, _, _), samples in data.items() if op == operation for value in samples]
    xmin, xmax = math.log10(min(ns)), math.log10(max(ns))
    ymin, ymax = min(values), max(values)
    if ymin == ymax:
        ymin *= 0.9
        ymax *= 1.1
    left, right, top, bottom = PAD
    plot_w, plot_h = WIDTH - left - right, HEIGHT - top - bottom

    def x(n):
        return left + (math.log10(n) - xmin) / (xmax - xmin or 1) * plot_w

    def y(value):
        return top + (ymax - value) / (ymax - ymin) * plot_h

    lines = [f'<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{HEIGHT}" viewBox="0 0 {WIDTH} {HEIGHT}">', '<rect width="100%" height="100%" fill="white"/>', f'<text x="{left}" y="22" font-family="sans-serif" font-size="18">{svg_escape(operation)} — repeated sweep range and median</text>']
    for fraction in range(6):
        value = ymin + (ymax - ymin) * fraction / 5
        yy = y(value)
        lines.append(f'<line x1="{left}" y1="{yy:.1f}" x2="{WIDTH-right}" y2="{yy:.1f}" stroke="#dddddd"/>')
        lines.append(f'<text x="{left-8}" y="{yy+4:.1f}" text-anchor="end" font-family="sans-serif" font-size="11">{value:.2f}</text>')
    for n in ns:
        xx = x(n)
        lines.append(f'<line x1="{xx:.1f}" y1="{top}" x2="{xx:.1f}" y2="{HEIGHT-bottom}" stroke="#eeeeee"/>')
    lines.extend([f'<line x1="{left}" y1="{HEIGHT-bottom}" x2="{WIDTH-right}" y2="{HEIGHT-bottom}" stroke="black"/>', f'<line x1="{left}" y1="{top}" x2="{left}" y2="{HEIGHT-bottom}" stroke="black"/>', f'<text x="{left-52}" y="{top+plot_h/2:.1f}" transform="rotate(-90 {left-52} {top+plot_h/2:.1f})" font-family="sans-serif" font-size="12">ns/op</text>'])
    for n in ns:
        lines.append(f'<text x="{x(n):.1f}" y="{HEIGHT-bottom+18}" text-anchor="middle" font-family="sans-serif" font-size="10">{n:g}</text>')
    for index, design in enumerate(designs):
        shade, stroke = color(index) + "33", color(index)
        low = " ".join(f"{x(n):.1f},{y(min(data[(operation, design, n)])):.1f}" for n in ns)
        high = " ".join(f"{x(n):.1f},{y(max(data[(operation, design, n)])):.1f}" for n in reversed(ns))
        mid = " ".join(f"{x(n):.1f},{y(median(data[(operation, design, n)])):.1f}" for n in ns)
        lines.append(f'<polygon points="{low} {high}" fill="{shade}" stroke="none"/>')
        lines.append(f'<polyline points="{mid}" fill="none" stroke="{stroke}" stroke-width="2"/>')
        legend_y = 42 + index * 17
        lines.append(f'<line x1="{WIDTH-right-180}" y1="{legend_y}" x2="{WIDTH-right-160}" y2="{legend_y}" stroke="{stroke}" stroke-width="2"/>')
        lines.append(f'<text x="{WIDTH-right-154}" y="{legend_y+4}" font-family="sans-serif" font-size="11">{svg_escape(design)}</text>')
    lines.append('</svg>')
    path.write_text("\n".join(lines) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path, help="Markdown report path")
    parser.add_argument("runs", nargs="+", type=Path, help="two or more sweep run directories")
    parser.add_argument("--title", default="Repeated sweep report")
    args = parser.parse_args()
    if len(args.runs) < 2:
        fail("at least two runs are required; one run cannot support uncertainty reporting")

    loaded = [load_run(run) for run in args.runs]
    infos, point_sets = zip(*loaded)
    for field in ("sha", "host", "governor", "command"):
        values = {info[field] for info in infos}
        if len(values) != 1:
            fail(f"refusing incomparable runs: {field} differs: {', '.join(sorted(values))}")
    expected = set(point_sets[0])
    for run, points in zip(args.runs[1:], point_sets[1:]):
        if set(points) != expected:
            fail(f"{run}/sweep.csv: point grid differs from {args.runs[0]}/sweep.csv")

    data = defaultdict(list)
    for points in point_sets:
        for key, value in points.items():
            data[key].append(value)
    output = args.output
    assets = output.with_name(output.stem + "-assets")
    if assets.exists():
        shutil.rmtree(assets)
    assets.mkdir(parents=True)
    output.parent.mkdir(parents=True, exist_ok=True)
    operations = sorted({op for op, _, _ in data})
    for operation in operations:
        write_svg(assets / f"{operation}.svg", operation, data)

    largest_n = max(n for _, _, n in data)
    designs = sorted({design for _, design, _ in data})
    lines = [f"# {args.title}", "", "This report is generated by `scripts/publish-sweep-report.py` from repeated runs of the same benchmark command.", "The line is the pointwise median of each run's median; the shaded band is the minimum–maximum range across runs.", "It is descriptive uncertainty, **not** a confidence interval and not evidence for CV or consistency claims.", "", "## Provenance", "", f"- Runs: {len(args.runs)}", f"- SHA: `{infos[0]['sha']}`", f"- Dates: {min(info['date'] for info in infos)} through {max(info['date'] for info in infos)}", f"- Host: `{infos[0]['host']}`", f"- Governor: `{infos[0]['governor']}`", f"- Start load averages: {', '.join(info['load'] for info in infos)}", f"- Command: `{infos[0]['command']}`", "", "## Headline comparison", "", f"At the largest common point, N = {largest_n:,}. Values are ns/op as median [min–max] over {len(args.runs)} runs.", ""]
    for operation in operations:
        lines.extend([f"### {operation}", "", "| Design | ns/op | vs hashbrown |", "| --- | ---: | ---: |"])
        baseline_samples = data.get((operation, "hashbrown", largest_n))
        baseline = median(baseline_samples) if baseline_samples else None
        for design in designs:
            samples = data[(operation, design, largest_n)]
            point = f"{median(samples):.2f} [{min(samples):.2f}–{max(samples):.2f}]"
            ratio = "—" if baseline is None else f"{median(samples) / baseline:.2f}x"
            lines.append(f"| {design} | {point} | {ratio} |")
        lines.extend(["", f"![{operation} curve]({assets.name}/{operation}.svg)", ""])
    output.write_text("\n".join(lines))
    print(f"published {len(args.runs)} comparable runs ({infos[0]['sha']}) to {output}")


if __name__ == "__main__":
    main()
