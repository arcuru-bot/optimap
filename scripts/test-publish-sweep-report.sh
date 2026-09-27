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

# Refuse a same-SHA run when its build provenance differs.
bad="$tmp/bad"
cp -r "$fixture/run-b" "$bad"
python3 - "$bad/meta.json" <<'PY'
import json, sys
path = sys.argv[1]
with open(path) as source:
    meta = json.load(source)
meta["rustflags"] = "-C target-cpu=generic"
with open(path, "w") as output:
    json.dump(meta, output)
PY
if python3 "$project_dir/scripts/publish-sweep-report.py" "$tmp/bad.md" "$fixture/run-a" "$bad" 2>"$tmp/bad.err"; then
    echo 'publisher accepted incomparable rustflags' >&2
    exit 1
fi
grep -Fq 'rustflags differs' "$tmp/bad.err"

# Default (unset) RUSTFLAGS is a recorded value, not missing metadata: the
# nightly refresh publishes runs whose environment set no RUSTFLAGS.
default_a="$tmp/default-a"
default_b="$tmp/default-b"
cp -r "$fixture/run-a" "$default_a"
cp -r "$fixture/run-b" "$default_b"
python3 - "$default_a/meta.json" "$default_b/meta.json" <<'PY'
import json, sys
for path in sys.argv[1:]:
    with open(path) as source:
        meta = json.load(source)
    meta["rustflags"] = ""
    with open(path, "w") as output:
        json.dump(meta, output)
PY
printf '%s\n' --op lookup_miss --design-set baseline --max-n 1000 >"$default_a/sweep.args"
printf '%s\n' --op lookup_miss --design-set baseline --max-n 1000 >"$default_b/sweep.args"
python3 "$project_dir/scripts/publish-sweep-report.py" "$tmp/default.md" "$default_a" "$default_b" >/dev/null
test -s "$tmp/default.md"
grep -Eq '^- Command: `cargo bench --bench sweep -- --op lookup_miss --design-set baseline --max-n 1000`$' "$tmp/default.md"
echo 'publish-sweep-report default-flags control: PASS'

# A generation failure must preserve the existing report and asset tree.
echo sentinel >"$tmp/report.md"
echo sentinel >"$tmp/report-assets/sentinel"
if PUBLISH_SWEEP_FAIL_OPERATION=insert python3 "$project_dir/scripts/publish-sweep-report.py" "$tmp/report.md" "$fixture/run-a" "$fixture/run-b" 2>/dev/null; then
    echo 'publisher failure control unexpectedly succeeded' >&2
    exit 1
fi
grep -Fxq sentinel "$tmp/report.md"
grep -Fxq sentinel "$tmp/report-assets/sentinel"
echo 'publish-sweep-report hardening controls: PASS'
