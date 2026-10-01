# majordomus-covers: none
# The coverage measurement leaves a record, and the preflight judges coverage by it.
#
# `verification.coverage` was a constant: "unavailable", whatever had been measured, because
# scripts/rust-coverage measured and gated coverage and wrote nothing a program could read
# back. Every Cockpit showed it red for ever. Now the script records the measurement it makes
# itself at .ai/local/state/coverage/rust.json (the preflight's COVERAGE_RECORD) with the
# commit and tree it measured, and the check reads that record.
#
# The two halves are held together here, not in two places that could each agree with
# themselves: the real script writes the record and the real executable judges it. The
# instrumented suite is not run; a `cargo` on PATH answers `llvm-cov` by copying the
# hand-written export of case 132, so the case is deterministic and costs no rebuild.
#
#   1. nothing measured yet            -> unavailable
#   2. a gate run on a clean tree      -> the record names HEAD and clean, pass; verified
#   3. a commit after it               -> stale (a measurement of another commit)
#   4. a failing suite                 -> suite_exit recorded; failed, not verified
#   5. the tree edited during the run  -> recorded dirty; stale
#   6. --report                        -> outcome report; active
#   7. --from and --changed            -> measure what this script did not run, record nothing
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
"$MJ" init >/dev/null; "$MJ" update >/dev/null

REL="apps/majordomus-cli/src/sample.rs"
REC=".ai/local/state/coverage/rust.json"
mkdir -p "$(dirname "$REL")" scripts
cp "$ROOT/test/fixtures/coverage/sample.rs" "$REL"
cp "$ROOT/scripts/rust-coverage" scripts/rust-coverage; chmod +x scripts/rust-coverage
git add -A >/dev/null && git commit -qm base
grep -qxF '.ai/local/' .gitignore 2>/dev/null || git check-ignore -q "$REC" \
  || { echo "    the fixture does not ignore $REC, so recording would dirty it"; exit 1; }

# The export, its placeholder filename pointed at the fixture's sample.
FX="$T/../export-552.json"
SAMPLE="$T/$REL" python3 - "$ROOT/test/fixtures/coverage/export.json" "$FX" <<'PY'
import json, os, sys
data = json.load(open(sys.argv[1]))
target = os.environ["SAMPLE"]
for f in data["data"][0].get("files", []):
    if f["filename"] == "SAMPLE_ABS":
        f["filename"] = target
for fn in data["data"][0].get("functions", []):
    fn["filenames"] = [target if n == "SAMPLE_ABS" else n for n in fn.get("filenames", [])]
json.dump(data, open(sys.argv[2], "w"))
PY

# A cargo that runs no suite: `llvm-cov --version` answers, a measurement copies the export
# to --output-path, exits FAKE_CARGO_EXIT, and first appends to FAKE_CARGO_TOUCH if named.
BIN="$T/../bin-552"; mkdir -p "$BIN"
cat > "$BIN/cargo" <<SH
#!/bin/sh
[ "\$1 \$2" = "llvm-cov --version" ] && { echo "cargo-llvm-cov 0.0.0"; exit 0; }
out=""
while [ \$# -gt 0 ]; do [ "\$1" = --output-path ] && out="\$2"; shift; done
[ -n "\${FAKE_CARGO_TOUCH:-}" ] && echo "// edited mid-run" >> "\$FAKE_CARGO_TOUCH"
cp "$FX" "\$out"
exit "\${FAKE_CARGO_EXIT:-0}"
SH
chmod +x "$BIN/cargo"
cov() { PATH="$BIN:$PATH" scripts/rust-coverage "$@" >/dev/null 2>&1; }

MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
check_of() {   # <field>: verification.coverage's field in the preflight
  "$RB" env preflight --repo "$T" --format json | python3 -c 'import json,sys
d=json.load(sys.stdin)
print([c for s in d["sections"] for c in s["checks"] if c["id"]=="verification.coverage"][0][sys.argv[1]])' "$1"
}
expect_verdict() {   # <verdict> <why>
  local got; got="$(check_of verdict)" || { echo "    no preflight"; exit 1; }
  [ "$got" = "$1" ] || { echo "    coverage is '$got', expected '$1': $2"; echo "    $(check_of summary)"; exit 1; }
}
field() { python3 -c 'import json,sys; d=json.load(open(sys.argv[1])); print(d[sys.argv[2]])' "$REC" "$1"; }

# ---------------------------------------------------------------- 1. nothing measured
expect_verdict unavailable "no record exists"

# ---------------------------------------------------------------- 2. a clean gate run
cov || { echo "    the gate failed on the fixture"; exit 1; }
[ -f "$REC" ] || { echo "    the gate run left no record at $REC"; exit 1; }
[ "$(field commit)" = "$(git rev-parse HEAD)" ] || { echo "    the record does not name HEAD"; cat "$REC"; exit 1; }
[ "$(field working_tree)" = clean ] || { echo "    a clean run was recorded $(field working_tree)"; exit 1; }
[ "$(field outcome)" = pass ] || { echo "    the outcome is $(field outcome)"; exit 1; }
[ -z "$(git status --porcelain)" ] || { echo "    recording dirtied the tree:"; git status --porcelain; exit 1; }
expect_verdict verified "a passing measurement of HEAD on a clean tree"
check_of summary | grep -qF 'lines 50.00% (1/2)' || { echo "    the summary does not carry the measurement: $(check_of summary)"; exit 1; }

# ---------------------------------------------------------------- 3. another commit
echo x > other && git add other && git commit -qm next
expect_verdict stale "the record is about the previous commit"

# ---------------------------------------------------------------- 4. a failing suite
FAKE_CARGO_EXIT=101 cov || true
[ "$(field suite_exit)" = 101 ] || { echo "    the suite's exit was not recorded"; cat "$REC"; exit 1; }
expect_verdict failed "thresholds over a failing suite do not verify coverage"

# ---------------------------------------------------------------- 5. edited during the run
FAKE_CARGO_TOUCH="$T/other" cov || true
[ "$(field working_tree)" = dirty ] || { echo "    a tree edited mid-run was recorded $(field working_tree)"; exit 1; }
git checkout -q -- other
expect_verdict stale "the tree moved under the measurement"

# ---------------------------------------------------------------- 6. --report
cov --report || { echo "    --report failed"; exit 1; }
[ "$(field outcome)" = report ] || { echo "    --report recorded $(field outcome)"; exit 1; }
expect_verdict active "measured at HEAD, no threshold held"

# ---------------------------------------------------------------- 7. partial or foreign measurements
rm -f "$REC"
cov --from "$FX" || true
[ ! -e "$REC" ] || { echo "    --from recorded a tree it never measured"; exit 1; }
cov --changed HEAD || true
[ ! -e "$REC" ] || { echo "    --changed recorded a partial measurement as the crate's"; exit 1; }
expect_verdict unavailable "nothing this script ran itself was recorded"
