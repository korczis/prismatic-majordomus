# majordomus-covers: none
# A commit that a record or the site's data names is a commit the history holds (I3007).
# `scripts/ci/link-check --sources` decides, before any site is built, every commit/<sha> link
# written in a tracked file under .ai/repo/project, site/data or site/content-src, and every
# commit a data file hands a template to link (the key names come from the templates'
# `/commit/{{ … }}`), by the rules the built-page check uses. On 2026-10-18 the 0.30.0 top's derived
# data named a top that was never pushed and four reworded commits, and two issue records cited
# evidence commits that were rebased before landing; each was found only after a site build. Held
# here on a fixture repository of this case's own:
#
#   clean       a data commit and a record link that the history holds pass
#   data        a data field naming a commit outside the history fails, naming the file and field
#   record      a record linking a commit outside the history fails, naming the record
#   template    a field no template links is not decided, whatever it holds
#   shallow     a shallow clone cannot know, so it defers (exit 12), never a finding
. "$ROOT/test/lib.sh"
command -v python3 >/dev/null 2>&1 || skip "python3 is not installed"

S="$(mktemp -d "${TMPDIR:-/tmp}/mj3400.XXXXXX")"
trap 'rm -rf "$S"' EXIT
F="$S/repo"; mkdir -p "$F"
g() { git -C "$F" -c user.email=case@example.com -c user.name=case "$@"; }
check() { MJ_ROOT="$1" "$ROOT/scripts/ci/link-check" --sources; }
FORGE=https://github.com/example/fixture

g init -q -b master
mkdir -p "$F/site/templates" "$F/site/data/generated" "$F/.ai/repo/project/issues"
printf 'base_url = "https://example.org"\n' > "$F/site/config.toml"
printf '{"repository_url": "%s"}\n' "$FORGE" > "$F/site/data/generated/project.json"
printf '<a href="%s/commit/{{ delivery.observed.commit }}">observed</a>\n' "$FORGE" > "$F/site/templates/index.html"
printf 'id: I0001\n' > "$F/.ai/repo/project/issues/I0001.yaml"
g add -A && g commit -qm 'the fixture'
HELD="$(g rev-parse HEAD)"
# a commit the history never holds: on a branch that is never merged
g checkout -q -b elsewhere && printf 'x\n' > "$F/x" && g add x && g commit -qm 'never merged'
GONE="$(g rev-parse HEAD)"
g checkout -q master

write() {   # <observed commit> <record link commit> <unlinked field commit>
  printf '{"observed": {"commit": "%s"}, "notes": {"sha": "%s"}}\n' "$1" "$3" > "$F/site/data/generated/delivery.json"
  printf 'id: I0001\nwhy: "see %s/commit/%s"\n' "$FORGE" "$2" > "$F/.ai/repo/project/issues/I0001.yaml"
  g add -A && g commit -qm "the data at $1" >/dev/null
}

# ---------------------------------------------------------------- clean
write "$HELD" "${HELD:0:12}" "$GONE"
expect_exit 0 check "$F" || exit 1
echo "    clean: a data commit and a record link the history holds pass"

# ---------------------------------------------------------------- template
# the same run: notes.sha held a commit outside the history, and no template links `sha`
expect_no_grep "$GONE" || { echo "    a field no template links was decided"; exit 1; }
echo "    template: a field no template links is not decided"

# ---------------------------------------------------------------- data
write "$GONE" "$HELD" "$HELD"
expect_exit 10 check "$F" || exit 1
expect_grep "site/data/generated/delivery.json:observed.commit: .*$GONE.*is not in the history" || exit 1
echo "    data: a data field naming a commit outside the history fails, naming the file and field"

# ---------------------------------------------------------------- record
write "$HELD" "$GONE" "$HELD"
expect_exit 10 check "$F" || exit 1
expect_grep "\.ai/repo/project/issues/I0001\.yaml: .*commit/$GONE.*is not in the history" || exit 1
echo "    record: a record linking a commit outside the history fails, naming the record"

# ---------------------------------------------------------------- shallow
write "$HELD" "$HELD" "$HELD"
git clone -q --depth 1 "file://$F" "$S/shallow"
expect_exit 12 check "$S/shallow" || exit 1
expect_grep "cannot be decided by a clone with no history" || exit 1
echo "    shallow: a clone that cannot know defers (exit 12) and reports no finding"
