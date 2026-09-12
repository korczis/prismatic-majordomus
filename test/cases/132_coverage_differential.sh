# majordomus-covers: none
# This case is the proof for claim new-code-is-covered (docs/CLAIMS.yaml): it is the
# test that claim names, and this line is where it names the claim back.
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
