# Repeated Sweep Reports

`bench-results/` is intentionally gitignored because it holds raw local measurements.
When two or more completed runs have the same SHA and measurement conditions, turn them into a check-in-ready mdBook page and SVG assets:

```bash
scripts/publish-sweep-report.py \
  docs/src/benchmarks/reports/2026-08-23-tomb.md \
  bench-results/runs/<run-a> bench-results/runs/<run-b>
```

The generated Markdown and its `-assets/` directory are ordinary repository files, so they can be reviewed in a diff.
Add the report to `SUMMARY.md` to make it part of the book: mdBook copies the SVG assets either way, but it silently skips any page `SUMMARY.md` does not list.
The publisher refuses runs whose SHA, host, CPU governor, command, or CSV point grid differs.
It records SHA, dates, host, governor, start load, and command in the report.

For every operation, the report gives a headline comparison at the largest common N and an SVG curve.
The curve's line is the pointwise median of each run's median, while its shaded band is the pointwise minimum–maximum across the runs.
That band is descriptive uncertainty only: it is neither a confidence interval nor evidence for a CV or consistency claim.

The sweep runner records the command and starting load in new `meta.json` files.
The publisher also accepts the older `bench`, `machine`, `cpu_governor`, `created`, and `commit` spellings of those fields.
It still requires full build provenance, so pre-existing runs whose `meta.json` lacks `rustc`, `rustflags`, `git_dirty_files`, or `kernel` are refused rather than published without it.
Raw input remains local; commit only a report when its recorded conditions support the claim.

Run `just report-test` to regenerate the committed smoke CSV fixture twice and compare both Markdown and SVG outputs byte-for-byte.
