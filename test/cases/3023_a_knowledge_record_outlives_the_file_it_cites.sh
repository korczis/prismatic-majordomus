# majordomus-covers: knowledge check
# A knowledge record's file provenance survives the file's deletion (I1987, evidence
# historical_file_reference_case).
#
# OSCILLA 5467193: two curated notes named file:tests/engine.cjs and file:tests/smoke.cjs in
# derived_from, its V2 removed both files, and `knowledge check` failed — the citation was
# true when it was written and the only fix was to rewrite the evidence. A `file:` or `test:`
# reference the tree no longer holds now resolves through git history, and is named as
# historical rather than passed in silence. Held here, against the shell check and the Rust
# resolver alike:
#
#   1  a curated note cites a file, the file is deleted and the deletion committed:
#      `knowledge check` passes and names the reference historical, in text and in --json
#   2  the same reference pinned as file:<path>@<commit> resolves at that commit
#   3  `knowledge record` (the Rust resolver) answers historical, with the blob as target
#   4  a file no commit ever held still fails, and so does a pin at a commit that did not hold it
. "$ROOT/test/lib.sh"

unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID

"$MJ" init >/dev/null; "$MJ" update >/dev/null
git add -A >/dev/null 2>&1; git commit -qm "the layer" >/dev/null 2>&1 || true

CUR=.ai/repo/knowledge/curated
mkdir -p "$CUR" tests

# note ID REFERENCE...
note() {
  local id="$1"; shift
  {
    printf -- '---\nschema: knowledge/v1\nid: %s\nkind: knowledge\nclass: fact\n' "$id"
    printf 'title: "The engine is exercised by its own tests"\ndescription: "Written while the tests existed."\n'
    printf 'status: verified\nepistemics: observed\ndate: 2026-10-04\ntags:\n  - provenance\n'
    printf 'provenance:\n  origin: authored\n  derived_from:\n'
    local r; for r in "$@"; do printf '    - %s\n' "$r"; done
    printf -- '---\n\n# The engine is exercised by its own tests\n\nWritten while the tests existed.\n'
  } > "$CUR/$id.md"
}

# the file exists, the note cites it, both are committed
printf 'the engine\n' > tests/engine.cjs
note engine-is-tested file:tests/engine.cjs
git add -A >/dev/null; git commit -qm "an engine, and a note about it" >/dev/null
ADDED="$(git rev-parse --short=10 HEAD)"
expect_exit 0 "$MJ" knowledge check || { echo "    the note was refused while its file existed"; exit 1; }
expect_no_grep 'historical' || { echo "    a reference the tree holds was called historical"; exit 1; }

# ---------------------------------------------------------------- 1. the file is deleted
git rm -q tests/engine.cjs
git commit -qm "V2 removes the engine's tests" >/dev/null
REMOVED="$(git rev-parse HEAD)"
[ ! -e tests/engine.cjs ] || { echo "    the fixture still holds the file"; exit 1; }
expect_exit 0 "$MJ" knowledge check \
  || { echo "    a note whose cited file was deleted later failed knowledge check"; exit 1; }
expect_grep 'engine-is-tested.*historical.*file:tests/engine\.cjs' \
  || { echo "    the historical reference was passed without being named"; exit 1; }
expect_grep '^knowledge check: 1 record\(s\), 0 failing' || exit 1
expect_exit 0 "$MJ" --json knowledge check || exit 1
expect_grep '"historical":\[\{"path":"[^"]*engine-is-tested\.md","reference":"file:tests/engine\.cjs","field":"provenance\.derived_from\.0"\}\]' \
  || { echo "    --json does not list the historical reference"; exit 1; }

# ---------------------------------------------------------------- 2. pinned at a commit
note engine-is-tested "file:tests/engine.cjs@$ADDED"
git add -A >/dev/null; git commit -qm "pin the citation" >/dev/null
expect_exit 0 "$MJ" knowledge check || { echo "    a citation pinned at the commit that held the file failed"; exit 1; }
expect_grep "historical.*file:tests/engine\\.cjs@$ADDED" || exit 1

# ---------------------------------------------------------------- 3. the Rust resolver agrees
note engine-is-tested file:tests/engine.cjs
git add -A >/dev/null; git commit -qm "unpin it" >/dev/null
RB="$(rust_bin)" || rust_bin_exit $?
# the executable reads the kinds from the tool it ships with, as every Rust case gives it
rec() { MAJORDOMUS_SHARE="$ROOT/share" "$RB" knowledge record engine-is-tested; }
expect_exit 0 rec \
  || { echo "    knowledge record could not read the note"; exit 1; }
expect_grep "derived_from file:tests/engine\\.cjs  historical  -> $REMOVED\\^:tests/engine\\.cjs" \
  || { echo "    the Rust resolver does not answer historical with the blob that holds the file"; exit 1; }

# ---------------------------------------------------------------- 4. what still fails
SHORT="$(printf '%s' "$REMOVED" | cut -c1-10)"
for ref in file:tests/smoke.cjs "file:tests/engine.cjs@$SHORT" 'test:tests/*.cjs'; do
  note engine-is-tested "$ref"
  git add -A >/dev/null
  expect_exit 10 "$MJ" knowledge check || { echo "    $ref resolved, and nothing ever held it there"; exit 1; }
  expect_grep "FAIL +knowledge .*engine-is-tested.*resolves to nothing" || exit 1
  expect_exit 0 rec || exit 1
  expect_grep "  missing  \\(no such path in the tree or in its history" \
    || { echo "    the Rust resolver did not refuse $ref"; exit 1; }
done

echo "    a note whose cited file was deleted passes knowledge check, named historical (plain and pinned), and the Rust resolver agrees; a file that never existed, a pin where it was gone and a glob still fail"
