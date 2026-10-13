# majordomus-covers: doctor
# A parser that cannot run says nothing about the file it was given.
#
# `doctor` checks that every schema the tool ships parses as JSON. It asked python3 when
# python3 was on PATH, and read any failure of it as the file's. A version manager's shim is
# on PATH whether or not it can start anything: asdf answers "No version is set for command
# python3" and exits non-zero in every directory with no pinned Python, and pyenv does the
# same. On such a machine the advertised first run — install, init, update, doctor — failed
# on the tool's own data: "52 schema(s) do not parse as JSON", with jq sitting beside it
# unasked. Found validating the 0.17.0 release artifact on a fresh repository, 2026-10-08.
#
# This is rule project.a-failed-read-is-not-an-empty-answer in another shape: a reader that
# failed was taken for a reading. What is held: with a python3 that cannot run and a jq that
# can, the schemas are judged by jq and found valid; a file that really is not JSON is still
# refused; and with no parser that runs, doctor says the parse was not asked instead of
# calling the schemas broken or calling them valid.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
git add -A && git commit -qm base

# a python3 as a version manager leaves it where no version is pinned
SHIMS="$(mktemp -d "${TMPDIR:-/tmp}/mj999.XXXXXX")"; trap 'rm -rf "$SHIMS"' EXIT
mkdir -p "$SHIMS/python" "$SHIMS/both"
shim() { printf '#!/bin/sh\necho "No version is set for command %s" >&2\nexit 126\n' "$2" > "$1/$2"; chmod +x "$1/$2"; }
shim "$SHIMS/python" python3
shim "$SHIMS/both" python3; shim "$SHIMS/both" jq
command -v jq >/dev/null 2>&1 || skip "no jq: the fallback this case holds cannot be observed"

# --- python3 cannot run, jq can: the schemas are judged, and found valid
PATH="$SHIMS/python:$PATH" "$MJ" doctor > doctor.txt 2>&1 || true
expect_no_grep 'do not parse as JSON' doctor.txt
expect_grep '^OK +schema +share/schemas .* each valid JSON' doctor.txt

# --- and judged for real: jq refuses what is not JSON, and reads a field python3 could not
printf '{"$id": "majordomus.sample/v1"}\n' > good.json; printf '{"$id": \n' > bad.json
out="$(PATH="$SHIMS/python:$PATH" MJ_BIN_DIR="$ROOT/bin" MJ_LIB_DIR="$ROOT/lib" MJ_VERSION=0 bash -c '
  set -eu; . "$MJ_LIB_DIR/common.sh"; . "$MJ_LIB_DIR/doctor.sh"
  mj_json_ok good.json && echo good
  mj_json_ok bad.json || echo refused
  printf "%s\n" "$MJ_JSON_PARSER"
  printf "%s\n" "$(mj_json_string good.json "\$id")"
' 2>&1)"
[ "$out" = "$(printf 'good\nrefused\njq\nmajordomus.sample/v1')" ] \
  || { echo "    with python3 unable to run, the readers answered:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }

# --- no parser runs at all: nothing is called broken, and nothing is called valid
PATH="$SHIMS/both:$PATH" "$MJ" doctor > doctor.txt 2>&1 || true
expect_no_grep 'do not parse as JSON' doctor.txt
expect_no_grep '^OK +schema +share/schemas' doctor.txt
expect_grep '^INFO +schema +share/schemas .* none was parsed' doctor.txt
rm -f good.json bad.json doctor.txt
