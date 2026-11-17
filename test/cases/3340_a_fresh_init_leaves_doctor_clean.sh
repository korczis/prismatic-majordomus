# majordomus-covers: init doctor update
# A repository that adopts the tool the way an adopter does (init, update, the two hook lines)
# is clean by doctor's account at once (I2941). Doctor's bootstrap check fails a README that does
# not name AGENTS.md (majordomus.bootstrap-integrity), and init never said so, so every first
# adopter with a README met one unexplained FAIL. A shared init now adds one paragraph naming
# AGENTS.md to a README that lacks it, once, leaving the rest of the README byte for byte; it
# creates no README where there is none, and a personal layer leaves the project's README alone.
. "$ROOT/test/lib.sh"
git config user.email t@example.com; git config user.name t

adopt() {   # in the current directory: init, update and the two hook lines, as init advises
  "$MJ" init > init.out 2>&1 || { echo "    init failed:"; sed 's/^/      /' init.out; exit 1; }
  mkdir -p .git/hooks
  printf '#!/usr/bin/env bash\nmajordomus doctor\n' > .git/hooks/pre-commit
  printf '#!/usr/bin/env bash\nmajordomus finish --check\n' > .git/hooks/pre-push
  chmod +x .git/hooks/pre-commit .git/hooks/pre-push
  "$MJ" update > /dev/null 2>&1 || { echo "    update failed"; exit 1; }
}
PATH="$(dirname "$MJ"):$PATH"; export PATH

# ---------------------------------------------------------------- 1. a README that names nothing
printf '# demo\n\nA project that has never heard of an AI layer.' > README.md   # no final newline
git add README.md && git commit -qm "the project as it was"
before="$(cat README.md)"
adopt
expect_exit 0 "$MJ" doctor || { echo "    doctor is not clean after a fresh adoption"; exit 1; }
expect_no_grep '^FAIL' || exit 1
expect_grep 'OK +bootstrap +README.md — names AGENTS.md' || exit 1
expect_grep 'README.md:AGENTS.md' init.out || { echo "    init did not say it named AGENTS.md in README.md"; exit 1; }
[ "$(head -c "${#before}" README.md)" = "$before" ] || { echo "    init changed what the README already said"; exit 1; }
grep -q '^## AI agents$' README.md && grep -q '\[AGENTS.md\](AGENTS.md)' README.md \
  || { echo "    the README does not carry the paragraph naming AGENTS.md:"; sed 's/^/      /' README.md; exit 1; }
grep -q 'A project that has never heard of an AI layer\.$' README.md \
  || { echo "    the README's last line was fused with what init appended"; exit 1; }

# ---------------------------------------------------------------- 2. once, whatever the reruns
"$MJ" init --extend > init.out 2>&1 || { echo "    init --extend failed"; exit 1; }
[ "$(grep -c '^## AI agents$' README.md)" = 1 ] || { echo "    a rerun added the paragraph again"; exit 1; }
expect_no_grep 'README.md:AGENTS.md' init.out || { echo "    a rerun claimed to name AGENTS.md again"; exit 1; }

# ---------------------------------------------------------------- 3. a README that already names it
R2="$T/named"; mkdir -p "$R2"; cd "$R2" || exit 1
git init -q . && git config user.email t@example.com && git config user.name t
printf '# named\n\nAgents read AGENTS.md first.\n' > README.md
cp README.md "$T/named.before"
adopt
cmp -s README.md "$T/named.before" || { echo "    init touched a README that already names AGENTS.md"; exit 1; }

# ---------------------------------------------------------------- 4. no README: none is created
R3="$T/bare"; mkdir -p "$R3"; cd "$R3" || exit 1
git init -q . && git config user.email t@example.com && git config user.name t
adopt
[ ! -e README.md ] || { echo "    init created a README"; exit 1; }
expect_exit 0 "$MJ" doctor || { echo "    doctor is not clean in a repository with no README"; exit 1; }

# ---------------------------------------------------------------- 5. a personal layer leaves the project's README alone
R4="$T/personal"; mkdir -p "$R4"; cd "$R4" || exit 1
git init -q . && git config user.email t@example.com && git config user.name t
printf '# shared project\n' > README.md && git add README.md && git commit -qm base
"$MJ" init --personal > init.out 2>&1 || { echo "    init --personal failed:"; sed 's/^/      /' init.out; exit 1; }
[ "$(cat README.md)" = "# shared project" ] || { echo "    a personal init wrote into the project's README"; exit 1; }
echo "    a fresh adoption is doctor-clean: a README gains one paragraph naming AGENTS.md, once; a README that names it, no README, and a personal layer are left alone"
