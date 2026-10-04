# majordomus-covers: finish
# majordomus-negative: finish
# An episode that ends is not work that was accepted, and a completion that is refused
# advances nothing (ADR 0106: the obligation is owed by accepted work, never by a boundary).
#
# Two facts are kept apart that a naive version bump conflates. A provider's session ending
# — the tab closed, the terminal died, the context compacted — closes the session envelope
# (`capture session --event end`): it records the episode and leaves an active task to its
# continuation, and it must not advance the version or mark the work completed. A
# `finish --outcome completed` whose verification fails is not an accepted completion either:
# it is refused, and it must leave no `release.advanced` in the ledger, no completed task and
# no advanced version behind it that a later reader would take for completed work.
#
#   episode end, work uncommitted        no advance; the task is handed over, not finished
#   finish completed, verification fails REFUSED, no advance recorded, version unchanged
#   the same finish, verification passes 1.4.0 -> 1.5.0, exactly one release.advanced
. "$ROOT/test/lib.sh"
BIN="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_BIN="$BIN" MAJORDOMUS_SHARE="$ROOT/share"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj872.XXXXXX")"; trap 'rm -rf "$S"' EXIT
"$MJ" init >/dev/null; "$MJ" update >/dev/null
printf '# Objective\n\nx\n\n# Current State\n\nx\n\n# Next Action\n\nx\n' > "$S/note.md"

declared() { sed -n 's/^version = "\(.*\)"$/\1/p' apps/majordomus-cli/Cargo.toml | head -n 1; }
ledger() { grep -c "\"event\":\"$1\"" .ai/local/state/ledger.jsonl 2>/dev/null || true; }

mkdir -p apps/majordomus-cli lib
printf '[package]\nname = "majordomus-cli"\nversion = "1.4.0"\n' > apps/majordomus-cli/Cargo.toml
printf '[[package]]\nname = "majordomus-cli"\nversion = "1.4.0"\n' > apps/majordomus-cli/Cargo.lock
echo 'a() { :; }' > lib/a.sh
sed -i.bak 's/^    - note_present$/    - note_present\n    - version_advanced/' .ai/repo/policy.yaml
printf '\nrelease:\n  cadence: minor\n' >> .ai/repo/policy.yaml
rm -f .ai/repo/policy.yaml.bak
git add -A >/dev/null && git commit -qm base
git update-ref refs/remotes/origin/master HEAD
git checkout -qb feature/a

# --------------------------------------------------- an episode ends with work on the bench
expect_exit 0 "$MJ" start "work interrupted by the end of an episode" --scope lib/,apps/,docs/,share/,site/
echo 'b() { :; }' >> lib/a.sh
printf '{"session_id":"s-872","hook_event_name":"SessionEnd","reason":"other"}\n' \
  | "$MJ" capture session --provider claude-code --event end >/dev/null 2>&1 || true
[ "$(declared)" = 1.4.0 ] || { echo "    the end of an episode advanced the version to $(declared)"; exit 1; }
[ "$(ledger release.advanced)" = 0 ] || { echo "    the end of an episode recorded an advance"; exit 1; }
[ "$(ledger task.finished)" = 0 ] || { echo "    the end of an episode finished the task"; exit 1; }
grep -q '^outcome: handed_over' .ai/local/state/current.yaml \
  || { echo "    the end of an episode did not hand the task over:"; cat .ai/local/state/current.yaml; exit 1; }

# the next session picks the work up as a task of its own
expect_exit 0 "$MJ" start "the interrupted work, resumed" --scope lib/,apps/,docs/,share/,site/

# --------------------------------------------------- a refused completion advances nothing
expect_exit 10 "$MJ" finish --outcome completed --verify-command "false" --note "$S/note.md"
[ "$(ledger release.advanced)" = 0 ] || { echo "    a refused completion recorded release.advanced"; exit 1; }
[ "$(ledger task.finished)" = 0 ] || { echo "    a refused completion recorded task.finished"; exit 1; }
[ "$(declared)" = 1.4.0 ] || { echo "    a refused completion left the version advanced to $(declared)"; exit 1; }

# --------------------------------------------------- the same work, accepted, advances once
expect_exit 0 "$MJ" finish --outcome completed --verify-command "test -d .ai" --note "$S/note.md"
[ "$(declared)" = 1.5.0 ] || { echo "    accepted work left the version at $(declared)"; exit 1; }
[ "$(ledger release.advanced)" = 1 ] || { echo "    expected one release.advanced, found $(ledger release.advanced)"; exit 1; }

echo "    episode end: nothing; refused completion: nothing; accepted: 1.4.0 -> 1.5.0 once"
