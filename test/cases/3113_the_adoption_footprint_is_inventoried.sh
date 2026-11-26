# majordomus-covers: init update capture
# What a default adoption leaves outside .ai/ and .majordomus/ is measured here rather than
# remembered (I1101, ADR 0169). A fresh repository is adopted with the three commands the
# adoption guide names -- init, update, capture install -- and every path that then exists
# outside the two namespaces is compared with the list below, which is the measured footprint
# docs/TWO_FOLDER_CONSOLIDATION.md records. A writer that starts leaving a new path behind
# fails this case until the inventory names it; a path the inventory names and nothing writes
# any more fails it too. The personal layer is measured the same way: git sees nothing, and
# the exclude block that hides the layer names every path the layer wrote.
# claims: none
. "$ROOT/test/lib.sh"

inventory="$ROOT/docs/TWO_FOLDER_CONSOLIDATION.md"
[ -f "$inventory" ] || { echo "    the footprint inventory $inventory is missing"; exit 1; }

# The footprint of init + update + capture install, outside .ai/ and .majordomus/. A Claude
# Code mod is a directory the provider owns the inside of, so it is listed as its directory.
expected='.claude/hooks/majordomus-capture
.claude/hooks/majordomus-session-compact
.claude/hooks/majordomus-session-end
.claude/hooks/majordomus-session-start
.claude/settings.json
.claude/skills/majordomus-brief/
.claude/skills/majordomus-handover/
.gitignore
AGENTS.md
CLAUDE.md
GEMINI.md'

# Every path git can see, ignored ones included, outside the two namespaces; a mod directory
# collapsed to itself. The repository started with one empty commit, so all of it is new.
footprint() {
  git status --porcelain --ignored -uall \
    | sed -e 's/^...//' -e 's/^"//' -e 's/"$//' \
    | grep -v -e '^\.ai/' -e '^\.majordomus/' \
    | sed -E 's#^(\.claude/skills/[^/]+/).*#\1#' \
    | LC_ALL=C sort -u
}
# The git directory, with the four merge-driver keys update declares (I4770) taken out: they
# are the adoption's one write there, measured on their own below.
merge_keys='merge.majordomus-derived.driver
merge.majordomus-derived.name
merge.majordomus-regions.driver
merge.majordomus-regions.name'
gitdir_state() {
  { git config --local --list | grep -v '^merge\.majordomus-\(derived\|regions\)\.' || true
    printf '%s\n' '--- exclude'; cat .git/info/exclude 2>/dev/null
    printf '%s\n' '--- hooks'; ls .git/hooks 2>/dev/null; } | cksum
}

# --- a shared adoption: these paths and no others, and in the git directory only the drivers
before="$(gitdir_state)"
expect_exit 0 "$MJ" init
expect_exit 0 "$MJ" update
expect_exit 0 "$MJ" capture install
observed="$(footprint)"
if [ "$observed" != "$expected" ]; then
  echo "    the adoption footprint is not the one the inventory records"
  diff <(printf '%s\n' "$expected") <(printf '%s\n' "$observed") | sed 's/^/    /'
  exit 1
fi
[ "$(gitdir_state)" = "$before" ] \
  || { echo "    a shared adoption changed .git/config, .git/info/exclude or .git/hooks"; exit 1; }
observed_keys="$(git config --local --name-only --get-regexp '^merge\.majordomus-' | LC_ALL=C sort)"
if [ "$observed_keys" != "$merge_keys" ]; then
  echo "    the merge-driver keys of a shared adoption are not the ones the inventory records"
  diff <(printf '%s\n' "$merge_keys") <(printf '%s\n' "$observed_keys") | sed 's/^/    /'
  exit 1
fi
grep -qx '# >>> majordomus merge drivers (majordomus update) >>>' .git/info/attributes \
  || { echo "    update declared no merge-driver block in .git/info/attributes"; exit 1; }

# --- the inventory names every one of them, so the document cannot fall behind the code
printf '%s\n' "$expected" "$merge_keys" '.git/info/attributes' | while IFS= read -r p; do
  grep -qF -- "\`$p" "$inventory" \
    || { echo "    $inventory does not name \`$p\`, which a default adoption writes"; exit 1; }
done

# --- a personal adoption: git sees nothing, and the exclude block names everything written
mkdir personal; cd personal || exit 1
git init -q . && git config user.email t@example.com && git config user.name t
git commit -q --allow-empty -m init
expect_exit 0 "$MJ" init --personal
expect_exit 0 "$MJ" update
expect_exit 0 "$MJ" capture install
[ -z "$(git status --porcelain)" ] \
  || { echo "    a personal adoption is visible to git:"; git status --porcelain | sed 's/^/    /'; exit 1; }
block="$(sed -n '/^# >>> majordomus personal >>>$/,/^# <<< majordomus personal <<<$/p' .git/info/exclude)"
[ -n "$block" ] || { echo "    no personal block in .git/info/exclude"; exit 1; }
printf '%s\n' '.ai/' "$expected" | grep -v -x '\.gitignore' | sed 's#/$##' \
  | while IFS= read -r p; do
      printf '%s\n' "$block" | grep -qx -- "/$p/\{0,1\}" \
        || { echo "    the personal block does not record /$p"; exit 1; }
    done
[ ! -e .gitignore ] || { echo "    a personal adoption wrote .gitignore"; exit 1; }
