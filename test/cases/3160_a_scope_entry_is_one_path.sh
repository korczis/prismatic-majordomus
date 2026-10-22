# majordomus-covers: start context
# majordomus-negative: start
# A scope entry is one path, and a path `context` would refuse is refused at `start` (I2760).
# On 2026-10-15 a pasted prompt given as --scope was accepted as one path and recorded as
# its words, a lone `/` among them; `context` then died on that `/`, and with it every
# commit hook in the worktree. A record that already holds such an entry costs one line of
# the briefing, never the briefing.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib/auth && echo x > lib/auth/a.txt && git add . && git commit -qm base

# ---------------------------------------------------------------- 1. whitespace is not a path
expect_exit 2 "$MJ" start "pasted" --scope "lib/auth,Prepare this as a prompt / not a study"
expect_grep "scope entry 'Prepare this as a prompt / not a study' holds whitespace"
[ ! -e .ai/local/state/current.yaml ] || { echo "    a refused start wrote a task record"; exit 1; }

# ---------------------------------------------------------------- 2. what context would refuse
OUT="$(mktemp -d "${TMPDIR:-/tmp}/mj-3160.XXXXXX")"
trap 'rm -rf "$OUT"' EXIT
ln -s "$OUT" lib/elsewhere
expect_exit 2 "$MJ" start "through a link" --scope lib/elsewhere/x
expect_grep "scope entry 'lib/elsewhere/x' .*resolves outside the repository"
[ ! -e .ai/local/state/current.yaml ] || { echo "    a refused start wrote a task record"; exit 1; }
expect_exit 15 "$MJ" context resolve lib/elsewhere/x
expect_grep "context: refused-path 'lib/elsewhere/x' resolves outside the repository"
rm lib/elsewhere

# ---------------------------------------------------------------- 3. an old record degrades one line
expect_exit 0 "$MJ" start "fix auth" --scope lib/auth
# the shape the 2026-10-15 record had: a lone `/` among the paths, as only a hand or an
# older executable writes it
awk '{ print } $0 == "  - lib/auth" { print "  - /" }' .ai/local/state/current.yaml > "$OUT/current.yaml"
cat "$OUT/current.yaml" > .ai/local/state/current.yaml
expect_grep '^  - /$' .ai/local/state/current.yaml
expect_exit 0 "$MJ" context
expect_grep '^## CONTEXT DOCUMENTS'
expect_grep '^lib/auth  '
expect_grep "^/  refused: refused-path '/' \(absolute, or leaves the repository\)"
expect_no_grep 'context: refused-path'
exit 0
