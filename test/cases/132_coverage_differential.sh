# majordomus-covers: none
# Proves claim new-code-is-covered (docs/CLAIMS.yaml), whose `test:` names this case.
# The differential coverage gate of #214, driven against a synthetic, self-consistent
# fixture so it is deterministic and needs no instrumented rebuild. The fixture is a tiny
# source file (test/fixtures/coverage/sample.rs) and a hand-written llvm-cov export
# (test/fixtures/coverage/export.json) whose segments name lines that exist in it: line 3 is
# covered (count 7), line 6 is uncovered (count 0). This is the mandatory adversarial
# acceptance of #214 — a deliberately uncovered changed line must fail the gate, and the same
# line covered must pass it — proved without depending on the real crate's line numbering,
# which drifts under it.
#
# The gate resolves paths against its own repo root, so the case places the sample source at
# a crate path in a scratch git repo, rewrites the export's filename to point there, and
# synthesises diffs against a base commit with `git diff`.
. "$ROOT/test/lib.sh"

FX_SRC="$ROOT/test/fixtures/coverage/sample.rs"
FX_EXP="$ROOT/test/fixtures/coverage/export.json"
[ -f "$FX_SRC" ] && [ -f "$FX_EXP" ] || { echo "    the coverage fixture is missing"; exit 1; }

# A scratch checkout whose one crate source file is the sample, plus the two scripts the gate
# is. The sample lives at a real crate path so the gate's crate-source pathspec matches it.
W="$T/repo"
REL="apps/majordomus-cli/src/sample.rs"
mkdir -p "$W/$(dirname "$REL")" "$W/scripts/ci"
git -C "$W" init -q
git -C "$W" config user.email t@example.com
git -C "$W" config user.name t
cp "$FX_SRC" "$W/$REL"
cp "$ROOT/scripts/rust-coverage" "$W/scripts/rust-coverage"; chmod +x "$W/scripts/rust-coverage"
cp "$ROOT/scripts/ci/coverage-differential" "$W/scripts/ci/coverage-differential"; chmod +x "$W/scripts/ci/coverage-differential"
git -C "$W" add -A >/dev/null
git -C "$W" commit -qm base
BASE="$(git -C "$W" rev-parse HEAD)"

# The export's filename placeholder becomes the sample's absolute path in the scratch repo.
FX="$T/export.json"
W="$W" REL="$REL" python3 - "$FX_EXP" "$FX" <<'PY'
import json, os, sys
data = json.load(open(sys.argv[1]))
target = os.path.join(os.environ["W"], os.environ["REL"])
for f in data["data"][0].get("files", []):
    if f["filename"] == "SAMPLE_ABS":
        f["filename"] = target
for fn in data["data"][0].get("functions", []):
    fn["filenames"] = [target if n == "SAMPLE_ABS" else n for n in fn.get("filenames", [])]
json.dump(data, open(sys.argv[2], "w"))
PY

cov() { ( cd "$W" && MJ_ROOT="$W" MJ_COVERAGE_EXPORT="$FX" scripts/ci/coverage-differential "$1" ); }
touch_line() { python3 - "$W/$REL" "$1" <<'PY'
import sys
path, n = sys.argv[1], int(sys.argv[2])
lines = open(path).read().splitlines(keepends=True)
if n <= len(lines):
    lines[n-1] = lines[n-1].rstrip("\n") + "  // touched by 132\n"
    open(path, "w").writelines(lines)
PY
}

# ---------------------------------------------------------------- 1. no change is a pass
expect_exit 0 cov "$BASE"
expect_grep "no changed crate source lines"

# ---------------------------------------------------------------- 2. an uncovered changed line FAILS (the point of #214)
touch_line 6
expect_exit 10 cov "$BASE"
expect_grep "changed line is not covered"
expect_grep "$REL:6"
git -C "$W" checkout -- "$REL"

# ---------------------------------------------------------------- 3. a covered changed line PASSES
touch_line 3
expect_exit 0 cov "$BASE"
expect_grep "every changed executable line"
git -C "$W" checkout -- "$REL"

# ---------------------------------------------------------------- 4. an unresolvable base is UNKNOWN, not a pass
expect_exit 13 cov "0000000000000000000000000000000000000000"

# ---------------------------------------------------------------- 5. a non-crate change is held to nothing
echo "# a note appended by 132" >> "$W/README.md"
git -C "$W" add README.md >/dev/null
expect_exit 0 cov "$BASE"
expect_grep "no changed crate source lines"

# ------------------------------- 6. the script's own exit vocabulary is 0/10/12/13, never cargo's
#
# scripts/rust-coverage declares four exits, and scripts/ci/coverage-differential delegates
# its own to them. cargo llvm-cov exits 101 when a test fails; under `set -e` that status
# escaped the script, so the gate answered with cargo's error instead of a verdict. The
# guard keeps the vocabulary closed: an export that is missing or unreadable is exit 12,
# "the measurement could not be trusted", and never 101.
grep -q 'exit 12' "$ROOT/scripts/rust-coverage" \
  || { echo "    rust-coverage no longer declares exit 12"; exit 1; }
grep -q 'cov_status=\$?' "$ROOT/scripts/rust-coverage" \
  || { echo "    the cargo llvm-cov call is unguarded again: a failing suite exits 101"; exit 1; }
# The floor reads --from the export the coverage job's one instrumented run wrote, and it runs
# even when that run wrote none (.github/workflows/validate.yml) — so a missing, empty or torn
# export is the same 12 as an instrumentation that wrote nothing, never python's traceback
# and exit 1.
expect_exit 12 "$W/scripts/rust-coverage" --from "$T/no-such-export.json"
expect_grep "the measurement could not be trusted"
: > "$T/empty-export.json"
expect_exit 12 "$W/scripts/rust-coverage" --from "$T/empty-export.json"
printf '{"data": [' > "$T/torn-export.json"
expect_exit 12 "$W/scripts/rust-coverage" --from "$T/torn-export.json"

# ------------------------------- 7. the changed-line read refuses rather than reading nothing
#
# The changed lines arrive through `git diff | awk`. Without pipefail a failing git reports
# the pipeline as awk's success, the file comes out empty, and the gate then answers "every
# changed line is covered" about a diff it never read. A pass that means "could not ask" is
# the defect project.a-verdict-states-its-subject refuses.
grep -q 'set -o pipefail' "$ROOT/scripts/rust-coverage" \
  || { echo "    rust-coverage pipes under set -e without pipefail: an unread diff passes"; exit 1; }

# ------------------------------- 8. each measurement writes its export to a name of its own
#
# The export cargo llvm-cov writes is a mktemp file. BSD mktemp (macOS) replaces only the
# X's that end a template: `mj-cov.XXXXXX.json` stayed that literal name, every run shared
# one file, and a second run beside the first found it already there. Every template in the
# script must end in its X's; and, driven through a cargo stand-in that records where it was
# told to write, two measurements must write to two different files, neither literally named.
bad="$(grep -o 'mktemp "[^"]*"' "$ROOT/scripts/rust-coverage" | grep -v 'XXX"$' || true)"
[ -z "$bad" ] || { echo "    a mktemp template does not end in its X's: $bad"; exit 1; }
STUB="$T/stub"; LOG="$T/cargo-llvm-cov.log"; mkdir -p "$STUB" "$T/tmp"
cat > "$STUB/cargo" <<'SH'
#!/usr/bin/env bash
# the stand-in answers `llvm-cov --version`, and a measurement by writing the fixture export
# to the --output-path it was given, which it records
[ "$1" = llvm-cov ] || exit 0
[ "$2" = --version ] && { echo "cargo-llvm-cov stand-in"; exit 0; }
out=""; while [ $# -gt 0 ]; do [ "$1" = --output-path ] && out="$2"; shift; done
printf '%s\n' "$out" >> "$STUB_LOG"
cp "$STUB_EXPORT" "$out"
SH
chmod +x "$STUB/cargo"
for run in 1 2; do
  rc=0
  # the stand-in shadows cargo only; TMPDIR keeps the exports inside this case
  ( cd "$W" && env PATH="$STUB:$PATH" TMPDIR="$T/tmp" STUB_LOG="$LOG" STUB_EXPORT="$FX" \
      scripts/rust-coverage --report ) > "$T/cov-$run.out" 2>&1 || rc=$?
  [ "$rc" = 0 ] || { tail -5 "$T/cov-$run.out"; echo "    measurement $run exited $rc"; exit 1; }
done
[ "$(wc -l < "$LOG" | tr -d ' ')" = 2 ] || { cat "$LOG"; echo "    cargo llvm-cov was not asked twice"; exit 1; }
grep -q 'XXX' "$LOG" && { cat "$LOG"; echo "    the export was written to a literal template name"; exit 1; }
[ "$(sort -u "$LOG" | wc -l | tr -d ' ')" = 2 ] \
  || { cat "$LOG"; echo "    two measurements wrote to one export file"; exit 1; }
