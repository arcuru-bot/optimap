# Repeated Sweep Reports

`bench-results/` is intentionally gitignored because it holds raw local measurements.
When two or more completed runs have the same SHA and measurement conditions, turn them into a check-in-ready mdBook page and SVG assets:

```bash
scripts/publish-sweep-report.py \
  docs/src/benchmarks/reports/2026-08-23-tomb.md \
  bench-results/runs/<run-a> bench-results/runs/<run-b>
```

The generated Markdown and its `-assets/` directory are ordinary repository files, so they can be reviewed in a diff and rendered by mdBook.
The publisher refuses runs whose SHA, host, CPU governor, command, or CSV point grid differs.
It records SHA, dates, host, governor, start load, and command in the report.

For every operation, the report gives a headline comparison at the largest common N and an SVG curve.
The curve's line is the pointwise median of each run's median, while its shaded band is the pointwise minimum–maximum across the runs.
That band is descriptive uncertainty only: it is neither a confidence interval nor evidence for a CV or consistency claim.

The sweep runner records the command and starting load in new `meta.json` files.
Older runs are supported when their metadata already contains the equivalent `bench`, `machine`, `cpu_governor`, and `created` fields.
Raw input remains local; commit only a report when its recorded conditions support the claim.

Run `just report-test` to regenerate the committed smoke CSV fixture twice and compare both Markdown and SVG outputs byte-for-byte.
