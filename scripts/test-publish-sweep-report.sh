#!/usr/bin/env bash
# Deterministic smoke test for the check-in-ready repeated-sweep publisher.
set -euo pipefail

project_dir="$(cd "$(dirname "$0")/.." && pwd)"
fixture="$project_dir/tests/fixtures/sweep-report"
tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

python3 "$project_dir/scripts/publish-sweep-report.py" "$tmp/report.md" "$fixture/run-a" "$fixture/run-b"
cp "$tmp/report.md" "$tmp/first.md"
cp -r "$tmp/report-assets" "$tmp/first-assets"
python3 "$project_dir/scripts/publish-sweep-report.py" "$tmp/report.md" "$fixture/run-a" "$fixture/run-b"

cmp "$tmp/first.md" "$tmp/report.md"
diff -ru "$tmp/first-assets" "$tmp/report-assets"
grep -Fq 'Tomb | 19.00 [18.00–20.00] | 0.90x |' "$tmp/report.md"
grep -Fq 'shaded band is the minimum–maximum range' "$tmp/report.md"
test -s "$tmp/report-assets/insert.svg"
test -s "$tmp/report-assets/lookup_hit.svg"
echo 'publish-sweep-report fixture smoke test: PASS'
