# majordomus-covers: none
# The gate that refuses a tree naming the machine it was written on.
#
# `scripts/site-check` refuses a built site carrying one, and that is the gate that caught this
# — at the moment of deployment, with the text already on master and the deploy already red.
# The reasoning of I0908, an issue whose stated purpose is that "no absolute developer path
# exists anywhere in the image", named a home directory; it reached plan.json, reached the
# site, and broke master's deploy. The cost of the late gate is that it bills the person
# publishing rather than the person writing.
#
# The same fault has a second half the site gate never sees: `share/allow/*.txt` was committed
# twice naming the absolute path of the worktree the generator last ran in. `derive-check` is
# structurally blind to that — it regenerates in the same checkout and gets the same answer —
# so the committed tree is read instead.
#
# What must hold is that the gate is wired into core-check, that this repository passes it,
# and — the part worth a test — that it fails on each half. A gate that cannot fail is
# decoration, so both halves are shown failing against a repository that carries the fault.
#
# Three things were added when the site check was found passing blind. The pattern is one
# table, driven through both corpora, because two copies of one expression had both missed
# `Bob` and `first.last`. A subject the gate cannot read in full is a refusal, exit 12, never
# a pass: the site check's `grep -r … | head -1 | grep -q .` printed "no local paths" under
# grep's own complaint that it could not open a page. And site-check reads through this gate
# rather than carrying a pattern of its own.
. "$ROOT/test/lib.sh"

GATE="$ROOT/scripts/ci/no-machine-paths"

# ---------------------------------------------------------------- wired, and passing here
expect_grep 'scripts/ci/no-machine-paths' "$ROOT/scripts/ci/core-check"
expect_exit 0 "$GATE" "$ROOT"
expect_grep '^no-machine-paths: OK — none of [0-9]+ tracked file'

# ---------------------------------------------------------------- a fixture repository
R="$T/repo"; mkdir -p "$R/.ai/repo/project/issues" "$R/share/allow"
git -C "$R" init -q
git -C "$R" config user.email t@example.com
git -C "$R" config user.name Test
commit_fixture() {
  git -C "$R" add -A
  git -C "$R" -c core.hooksPath=/dev/null commit -qm "${1:-fixture}"
}

printf 'why: "A Machine has no developer'"'"'s checkout."\n' > "$R/.ai/repo/project/issues/I0001.yaml"
printf '# GENERATED FILE — DO NOT EDIT DIRECTLY\n# Source: the document schema `share/schemas/majordomus/rule/rule.v1.proto`\n' \
  > "$R/share/allow/rule.txt"
commit_fixture clean
expect_exit 0 "$GATE" "$R"

# ------------------------------------------------- it fails on an authored document
printf 'why: "A Machine has no %s/dev."\n' "/Users/somebody" > "$R/.ai/repo/project/issues/I0001.yaml"
commit_fixture "authored fault"
expect_exit 1 "$GATE" "$R"

printf 'why: "A Machine has no developer'"'"'s checkout."\n' > "$R/.ai/repo/project/issues/I0001.yaml"
commit_fixture "authored reworded"
expect_exit 0 "$GATE" "$R"

# ------------------------------------------------- and on a generated artifact
# The shape the leak actually had: a banner naming its source schema by the absolute path
# of the worktree the generator ran in, rather than by a repository-relative one.
printf '# GENERATED FILE — DO NOT EDIT DIRECTLY\n# Source: the document schema `%s/dev/repo/share/schemas/majordomus/rule/rule.v1.proto`\n' \
  "/Users/somebody" > "$R/share/allow/rule.txt"
commit_fixture "generated leak"
expect_exit 1 "$GATE" "$R"

printf '# GENERATED FILE — DO NOT EDIT DIRECTLY\n# Source: the document schema `share/schemas/majordomus/rule/rule.v1.proto`\n' \
  > "$R/share/allow/rule.txt"
commit_fixture "generated relative"
expect_exit 0 "$GATE" "$R"

# ------------------------------------------------- and where the two pathspecs used to miss
# site/content/ is derived and was covered by neither of the two steps this gate replaced:
# a leak there reached publication before anything noticed.
mkdir -p "$R/site/content/docs"
printf 'The checkout lives at %s/dev/repo.\n' "/Users/somebody" > "$R/site/content/docs/page.md"
commit_fixture "derived site content"
expect_exit 1 "$GATE" "$R"

printf 'The checkout lives in a developer'"'"'s working directory.\n' > "$R/site/content/docs/page.md"
commit_fixture "derived site content reworded"
expect_exit 0 "$GATE" "$R"

# ------------------------------------------------- the pattern: one table, both corpora
# Caught: an account name of either case, with a digit, a dot or a hyphen; a Linux home with
# no `dev/` after it; a home named without a trailing slash; a file URL. The doctest
# placeholder that reached the published API reference is on this side on purpose — a
# placeholder spelled like a home directory cannot be told from one by any reader, so it is
# reworded where it is declared, never exempted here.
CATCH=(
  '/Users/korczis2/'
  '/Users/Bob/'
  '/Users/first.last/'
  '/Users/a-b/'
  '/home/anyone/'
  '"root": "/Users/x/repo"'
  '"root": "/home/bob"'
  'file:///Users/bob/notes'
)
# Not caught: the prose that documents this gate, both spellings of its pattern, a variable
# standing in for a home, a path no account owns, and a URL whose path happens to say home.
CLEAN=(
  'a home directory is `/Users/<name>/` or `/home/<name>/dev/`'
  'the old pattern /Users/[a-z]+/|/home/[a-z]+/dev/'
  'the new pattern (^|[^A-Za-z0-9._~%-])/(Users|home)/[A-Za-z0-9._-]+'
  'the /Users/ prefix and a bare /home/ directory'
  '$HOME/dev/app, ~/dev/app and /Users/$USER/dev'
  '"root": "/srv/repo"'
  'https://example.com/home/index and <a href="/docs/home/">'
)
mkdir -p "$R/docs"; B="$T/built"; mkdir -p "$B"
for row in "${CATCH[@]}"; do
  printf '%s\n' "$row" > "$R/docs/row.md"; commit_fixture "catch: $row"
  expect_exit 1 "$GATE" "$R"
  expect_grep '^docs/row\.md:1:'
  printf '<p>%s</p>\n' "$row" > "$B/index.html"
  expect_exit 1 "$GATE" --dir "$B"
  expect_grep '/index\.html:1:'
done
for row in "${CLEAN[@]}"; do
  printf '%s\n' "$row" > "$R/docs/row.md"; commit_fixture "clean: $row"
  expect_exit 0 "$GATE" "$R"
  printf '<p>%s</p>\n' "$row" > "$B/index.html"
  expect_exit 0 "$GATE" --dir "$B"
  expect_grep '^no-machine-paths: OK — none of 1 file\(s\) under '
done

# ------------------------------------------------- it refuses, rather than passes, when it cannot look
expect_exit 12 "$GATE" --dir "$T/never-built"
expect_grep 'never-built was not examined in full, so it is not reported clean: there is no directory there'
mkdir -p "$T/empty"
expect_exit 12 "$GATE" --dir "$T/empty"
expect_grep 'it holds no file'
git init -q "$T/nothing-tracked"
expect_exit 12 "$GATE" "$T/nothing-tracked"
expect_grep 'it tracks no file outside test/'
N="$(mktemp -d "${TMPDIR:-/tmp}/mj-58-norepo.XXXXXX")"; printf 'x\n' > "$N/f"
expect_exit 12 env GIT_CEILING_DIRECTORIES="$(dirname "$N")" "$GATE" "$N"
expect_grep 'git could not list what it tracks'
rm -rf "$N"

# A mode-000 entry is how an unread file is staged, and root reads one anyway, so there the
# staging itself is what cannot be done — said, rather than counted as a pass.
if [ "$(id -u)" = 0 ]; then
  echo "    skip: running as root, a mode-000 file is readable, so an unread one cannot be staged"
else
  bad=0
  # a directory find cannot walk
  mkdir -p "$T/walk/open" "$T/walk/shut"
  printf '<p>clean</p>\n' > "$T/walk/open/index.html"; printf '<p>/Users/Bob/</p>\n' > "$T/walk/shut/index.html"
  chmod 000 "$T/walk/shut"
  expect_exit 12 "$GATE" --dir "$T/walk" || bad=1
  expect_grep 'find could not walk all of it' || bad=1
  chmod 755 "$T/walk/shut"
  # a file grep cannot open, beside files it can and that are clean
  mkdir -p "$T/read"
  printf '<p>clean</p>\n' > "$T/read/index.html"; printf '<p>/Users/Bob/</p>\n' > "$T/read/shut.json"
  chmod 000 "$T/read/shut.json"
  expect_exit 12 "$GATE" --dir "$T/read" || bad=1
  expect_grep 'grep could not read all of it' || bad=1
  # and a leak it can read is still reported, with the unread file named beside it
  printf '<p>/Users/Bob/</p>\n' > "$T/read/index.html"
  expect_exit 1 "$GATE" --dir "$T/read" || bad=1
  expect_grep 'and the read was incomplete' || bad=1
  chmod 644 "$T/read/shut.json"
  [ "$bad" = 0 ]
fi

# ------------------------------------------------- site-check reads through this gate
# One pattern: the site check carries no expression of its own for a home directory, and no
# recursive read of the built site whose failure a pipeline can swallow.
expect_grep '"\$ROOT/scripts/ci/no-machine-paths" --dir "\$PUB"' "$ROOT/scripts/site-check"
expect_no_grep '/Users/\[' "$ROOT/scripts/site-check"
expect_no_grep 'grep -r[a-zA-Z]* [^|]*"\$PUB" *\|' "$ROOT/scripts/site-check"
