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

# ------------------------------- 7. a unit-test module in a file of its own is test code
#
# `src/<dir>/tests.rs`, declared by its parent as `#[cfg(test)] mod tests;`, is compiled only
# into the test build, as an inline `#[cfg(test)] mod tests { ... }` is, so its lines are test
# code and owe nothing. The attribute is in the parent: a gate reading the file alone called a
# test stub product code and demanded a test for it. Without the declaration, the same file is
# an ordinary module and its uncovered changed line fails.
W7="$T/repo7"; REL7="apps/majordomus-cli/src/demo/tests.rs"
mkdir -p "$W7/$(dirname "$REL7")" "$W7/scripts/ci"
git -C "$W7" init -q; git -C "$W7" config user.email t@example.com; git -C "$W7" config user.name t
cp "$FX_SRC" "$W7/$REL7"
printf 'pub fn demo() {}\n\n#[cfg(test)]\nmod tests;\n' > "$W7/apps/majordomus-cli/src/demo/mod.rs"
cp "$ROOT/scripts/rust-coverage" "$W7/scripts/rust-coverage"; chmod +x "$W7/scripts/rust-coverage"
cp "$ROOT/scripts/ci/coverage-differential" "$W7/scripts/ci/coverage-differential"; chmod +x "$W7/scripts/ci/coverage-differential"
git -C "$W7" add -A >/dev/null; git -C "$W7" commit -qm base
BASE7="$(git -C "$W7" rev-parse HEAD)"
FX7="$T/export7.json"
W="$W7" REL="$REL7" python3 - "$FX_EXP" "$FX7" <<'PY'
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
cov7() { ( cd "$W7" && MJ_ROOT="$W7" MJ_COVERAGE_EXPORT="$FX7" scripts/ci/coverage-differential "$1" ); }
python3 - "$W7/$REL7" <<'PY'
import sys
path = sys.argv[1]
lines = open(path).read().splitlines(keepends=True)
lines[5] = lines[5].rstrip("\n") + "  // touched by 132\n"
open(path, "w").writelines(lines)
PY
expect_exit 0 cov7 "$BASE7"
# the same file, no longer declared a test module: its uncovered changed line is owed
printf 'pub fn demo() {}\n\nmod tests;\n' > "$W7/apps/majordomus-cli/src/demo/mod.rs"
expect_exit 10 cov7 "$BASE7"
expect_grep "$REL7:6"

# ------------------------------- 9. a raw string across lines does not close a test module early
#
# `test_lines` finds the end of an inline `#[cfg(test)] mod tests { ... }` by matching braces.
# A multi-line raw string such as r#"{"graph": {"nodes": [ ... ]}}"#, whose braces are data,
# closed the module 63 lines early on 2026-10-22 (I3919): the panic branches after it were
# judged product code and failed the gate. The fixture's uncovered changed line (8) sits after
# such a string inside the test module and owes nothing; the same file without the attribute
# is an ordinary module and its line 8 is owed.
W9="$T/repo9"; REL9="apps/majordomus-cli/src/rawstr.rs"
mkdir -p "$W9/$(dirname "$REL9")" "$W9/scripts/ci"
git -C "$W9" init -q; git -C "$W9" config user.email t@example.com; git -C "$W9" config user.name t
cp "$ROOT/scripts/rust-coverage" "$W9/scripts/rust-coverage"; chmod +x "$W9/scripts/rust-coverage"
cp "$ROOT/scripts/ci/coverage-differential" "$W9/scripts/ci/coverage-differential"; chmod +x "$W9/scripts/ci/coverage-differential"
write_rawstr() {
  { [ "$1" = test ] && echo '#[cfg(test)]' || echo '// plain module'
    cat <<'RS'
mod tests {
    fn fixture() -> &'static str {
        r##"{"graph": {"nodes": [
            {"id": "x"}
        ], "edges": []}}"##
    }
    fn after() { panic!("test branch"); }
}
RS
  } > "$W9/$REL9"
}
write_rawstr test
git -C "$W9" add -A >/dev/null; git -C "$W9" commit -qm base
BASE9="$(git -C "$W9" rev-parse HEAD)"
# the export names the file; line 8 (the panic after the raw string) is uncovered
FX9="$T/export9.json"
W="$W9" REL="$REL9" python3 - "$FX9" <<'PY'
import json, os, sys
target = os.path.join(os.environ["W"], os.environ["REL"])
seg = lambda l, c, n, s: [l, c, n, s, s, False]
data = {"version": "2.0.1", "type": "llvm.coverage.json.export", "data": [{
    "files": [{"filename": target, "segments": [seg(8, 5, 0, True), seg(8, 40, 0, False)], "summary": {}}],
    "functions": [{"name": "after", "count": 0, "filenames": [target],
                   "regions": [[8, 5, 8, 40, 0, 0]]}]}]}
json.dump(data, open(sys.argv[1], "w"))
PY
cov9() { ( cd "$W9" && MJ_ROOT="$W9" MJ_COVERAGE_EXPORT="$FX9" scripts/ci/coverage-differential "$1" ); }
touch9() { python3 - "$W9/$REL9" <<'PY'
import sys
path = sys.argv[1]
lines = open(path).read().splitlines(keepends=True)
lines[7] = lines[7].rstrip("\n") + "  // touched by 132\n"
open(path, "w").writelines(lines)
PY
}
touch9
expect_exit 0 cov9 "$BASE9"
# the direct check: every line of the module, from its attribute to its closing brace, is test code
TL="$T/test-lines.out"
python3 - "$ROOT/scripts/rust-coverage" "$W9/$REL9" > "$TL" <<'PY'
import re, sys
src = open(sys.argv[1]).read()
code = src[src.index("RAW_OPEN = "):src.index("def declared_test_module")]
def read(path):
    return open(path).read().splitlines()
ns = {"re": re, "read": read}
exec(code, ns)
print(sorted(ns["test_lines"](sys.argv[2])))
PY
grep -q '^\[1, 2, 3, 4, 5, 6, 7, 8, 9\]$' "$TL" \
  || { cat "$TL"; echo "    test_lines closed the module early at a raw string"; exit 1; }
# the same file, no longer a test module: its uncovered changed line is owed
write_rawstr plain; touch9
expect_exit 10 cov9 "$BASE9"
expect_grep "$REL9:8"
