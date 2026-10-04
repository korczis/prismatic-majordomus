# majordomus-covers: finish
# majordomus-negative: finish
# A completion refused after its advance leaves every file the advance changed as it was
# (ADR 0106, amended 2026-10-04).
#
# `finish --outcome completed` pays an owed version obligation only once the rest of its
# contract holds: `release advance` writes the manifest and the lock, `generate distribution`
# projects share/version.txt (and the rest of the distribution's projections), and the
# obligation is read again. Until the amendment, a failure after the write refused the
# completion and left the advanced files behind — a tree declaring a version no accepted work
# paid for, which the next finish would have read as already satisfied. Each step that can
# fail after the write is made to fail here, through an executable that is the real one
# except for that step:
#
#   the re-read answers unverified        REFUSED, task.refused names the doctrine,
#                                         every changed file restored, no release.advanced
#   generate distribution fails after it  REFUSED the same way: a projection the contract
#                                         requires is not optional
#   nothing fails                         1.4.0 -> 1.5.0, one release.advanced whose event_id
#                                         names the subject, the trunk commit and the task
#   a replay                              writes nothing and appends nothing
. "$ROOT/test/lib.sh"
REAL="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null || skip "jq absent"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj877.XXXXXX")"; trap 'rm -rf "$S"' EXIT
export MJ877_ROOT="$PWD"
cat > "$S/majordomus" <<EOF
#!/usr/bin/env bash
# The real executable, except for the one step MJ877_FAIL names, and only once the manifest
# states the advanced version — so the steps before the write run for real.
advanced() { grep -qx 'version = "1.5.0"' "\$MJ877_ROOT/apps/majordomus-cli/Cargo.toml"; }
if [ "\${MJ877_FAIL:-}" = reread ] && [ "\$1 \$2" = "release obligation" ] && advanced; then
  printf '{"state":"unverified"}\n'; exit 12
fi
if [ "\${MJ877_FAIL:-}" = project ] && [ "\$1 \$2" = "generate distribution" ] && advanced; then
  echo "injected: generate distribution failed" >&2; exit 1
fi
exec "$REAL" "\$@"
EOF
chmod +x "$S/majordomus"
export MAJORDOMUS_BIN="$S/majordomus"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
printf '# Objective\n\nx\n\n# Current State\n\nx\n\n# Next Action\n\nx\n' > "$S/note.md"

declared() { sed -n 's/^version = "\(.*\)"$/\1/p' apps/majordomus-cli/Cargo.toml | head -n 1; }
ledger() { grep -c "\"event\":\"$1\"" .ai/local/state/ledger.jsonl 2>/dev/null || true; }
# every file that differs from the index, untracked ones included, with its digest: the
# whole of what a finish could have left behind in the work tree
tree_state() {
  git ls-files -z -m -o --exclude-standard | LC_ALL=C sort -z \
    | while IFS= read -r -d '' p; do
        if [ -f "$p" ]; then printf '%s %s\n' "$(git hash-object -- "$p")" "$p"; else printf 'absent %s\n' "$p"; fi
      done
}

cp -R "$ROOT/share" share
export MAJORDOMUS_SHARE="$PWD/share"
mkdir -p apps/majordomus-cli lib
printf '[package]\nname = "majordomus-cli"\nversion = "1.4.0"\n' > apps/majordomus-cli/Cargo.toml
printf '[[package]]\nname = "majordomus-cli"\nversion = "1.4.0"\n' > apps/majordomus-cli/Cargo.lock
printf 'share/version.txt merge=derived\n' > .gitattributes
echo 'a() { :; }' > lib/a.sh
sed -i.bak 's/^    - note_present$/    - note_present\n    - version_advanced/' .ai/repo/policy.yaml
printf '\nrelease:\n  cadence: minor\n' >> .ai/repo/policy.yaml
rm -f .ai/repo/policy.yaml.bak
git add -A >/dev/null && git commit -qm base
git update-ref refs/remotes/origin/master HEAD
TRUNK="$(git rev-parse HEAD)"
git checkout -qb feature/a

expect_exit 0 "$MJ" start "work whose advance is refused" --scope lib/,apps/,docs/,share/,site/
echo 'b() { :; }' >> lib/a.sh
BEFORE="$(tree_state)"

# refused_and_restored <what>: the finish just refused; nothing it advanced may remain
refused_and_restored() {
  [ "$(declared)" = 1.4.0 ] || { echo "    $1: the refused finish left the version at $(declared)"; exit 1; }
  grep -qx 'version = "1.4.0"' apps/majordomus-cli/Cargo.lock || { echo "    $1: the lock was left advanced"; exit 1; }
  local after; after="$(tree_state)"
  [ "$after" = "$BEFORE" ] || { echo "    $1: the work tree is not as it was before the advance:"; diff <(printf '%s\n' "$BEFORE") <(printf '%s\n' "$after"); exit 1; }
  [ "$(ledger release.advanced)" = 0 ] || { echo "    $1: a refused finish recorded release.advanced"; exit 1; }
  [ "$(ledger task.finished)" = 0 ] || { echo "    $1: a refused finish recorded task.finished"; exit 1; }
  grep -q '^outcome: active' .ai/local/state/current.yaml || { echo "    $1: the task is no longer active"; exit 1; }
  local last; last="$(grep '"event":"task.refused"' .ai/local/state/ledger.jsonl | tail -n 1)"
  [ "$(jq -r '.refused | join(",")' <<<"$last")" = majordomus.version-obligation ] \
    || { echo "    $1: task.refused does not name the version obligation: $last"; exit 1; }
  [ "$(jq -r '.contract["majordomus.version-obligation"]' <<<"$last")" = fail ] \
    || { echo "    $1: the recorded contract does not carry the refusal: $last"; exit 1; }
}

# --------------------------------------------------- the re-read after the advance fails
expect_exit 10 env MJ877_FAIL=reread "$MJ" finish --outcome completed --verify-command "test -d .ai" --note "$S/note.md"
expect_grep 'left the obligation unverified; the advance is undone'
refused_and_restored "a failed re-read"
[ "$(ledger task.refused)" = 1 ] || { echo "    expected one task.refused, found $(ledger task.refused)"; exit 1; }

# --------------------------------------------------- the projection fails after the advance
expect_exit 10 env MJ877_FAIL=project "$MJ" finish --outcome completed --verify-command "test -d .ai" --note "$S/note.md"
expect_grep 'generate distribution exited 1 after the advance to 1\.5\.0'
refused_and_restored "a failed projection"
[ "$(ledger task.refused)" = 2 ] || { echo "    expected two task.refused, found $(ledger task.refused)"; exit 1; }

# --------------------------------------------------- nothing fails: advanced once, and named
expect_exit 0 "$MJ" finish --outcome completed --verify-command "test -d .ai" --note "$S/note.md"
expect_grep 'version 1\.4\.0 -> 1\.5\.0'
[ "$(declared)" = 1.5.0 ] || { echo "    accepted work left the version at $(declared)"; exit 1; }
grep -qx 'version=1.5.0' share/version.txt || { echo "    share/version.txt does not state the advance"; exit 1; }
[ "$(ledger release.advanced)" = 1 ] || { echo "    expected one release.advanced, found $(ledger release.advanced)"; exit 1; }
TASK="$(sed -n 's/^id: //p' .ai/local/state/current.yaml)"
EVENT="$(grep '"event":"release.advanced"' .ai/local/state/ledger.jsonl)"
[ "$(jq -r .event_id <<<"$EVENT")" = "release.advanced/feature/a@$TRUNK/$TASK" ] \
  || { echo "    the advance is not named by its subject, trunk commit and task: $EVENT"; exit 1; }

# --------------------------------------------------- a replay appends nothing
expect_exit 15 "$MJ" finish --outcome completed --verify-command "test -d .ai" --note "$S/note.md"
expect_exit 0 "$REAL" release advance --repo .
expect_grep 'holds \(satisfied\).*nothing written'
[ "$(ledger release.advanced)" = 1 ] || { echo "    a replay appended another release.advanced"; exit 1; }

echo "    re-read fails: refused, restored; projection fails: refused, restored; accepted: one named advance"
