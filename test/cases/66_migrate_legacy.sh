# migrate moves a repository from the pre-.ai layout (.majordomus/ holding project data)
# to the portable .ai/ layer: explicitly, previewed, with a verified backup of the local
# state, never deleting what it does not know, and idempotent afterwards.
. "$ROOT/test/lib.sh"

# A legacy tree, built by hand: the tool no longer writes one, so the fixture is created on
# the .ai layout with real records and then moved into the shape the old layout had. Only
# the sections the old layout knew go there; the .ai-only sections do not exist yet.
make_legacy() {
  "$MJ" init >/dev/null; "$MJ" update >/dev/null
  mkdir -p lib && echo a > lib/a && git add -A && git commit -qm base
  "$MJ" start "old task" --scope lib >/dev/null
  echo b >> lib/a
  printf '# Objective\nold\n# Current State\nhalf\n# Next Action\nrest\n' | "$MJ" handover >/dev/null
  mkdir -p .majordomus
  for s in policy.yaml profiles prompts project; do mv ".ai/repo/$s" ".majordomus/$s"; done
  mv .ai/local/state .majordomus/state
  cp -R "$ROOT/share/skeleton/templates" .majordomus/templates
  # the old layout carried a provider body and monolithic templates that asked for it;
  # neither exists in the tool any more, and the body is what the old projections were
  mkdir -p .majordomus/providers
  printf '## How AI work runs here\n\nTen rules, once monolithic.\n' > .majordomus/providers/body.md
  for p in claude-code codex gemini generic; do printf '# %s\n\n{{BODY}}\n' "$p" > ".majordomus/providers/$p.tmpl"; done
  mkdir -p .majordomus/generated && printf 'stale\n' > .majordomus/generated/fingerprints.yaml
  rm -rf .ai
  sed -i.bak '/^\.ai\/local\/$/d' .gitignore; rm -f .gitignore.bak
  git add -A && git commit -qm legacy >/dev/null
}
# a fresh disposable repository to build one fixture in; the case's own is left alone
fresh_repo() {
  local d; d="$(mktemp -d "${TMPDIR:-/tmp}/mj-mig.XXXXXX")"
  cd "$d" && git init -q . && git config user.email t@example.com && git config user.name t \
    && git commit -q --allow-empty -m init
}
state_hashes() { (cd "$1" && find . -type f | LC_ALL=C sort | while read -r f; do printf '%s %s\n' "$(shasum -a 256 "$f" | cut -d' ' -f1)" "$f"; done); }
tree_hash() { { git status --porcelain; find . -path ./.git -prune -o -type f -print | LC_ALL=C sort | xargs shasum -a 256; } | shasum -a 256; }

# ---------------------------------------------------------------- the pure legacy tree
fresh_repo; make_legacy
[ -f .majordomus/policy.yaml ] && [ ! -e .ai ]
# a customised template moves; an unchanged one is dropped
printf '\ncustom line\n' >> .majordomus/templates/handover.md
# an uncommitted handover: dirty operational state travels too
cp "$(ls .majordomus/state/handovers/*.md | head -n 1)" .majordomus/state/handovers/99991231T000000Z--master--0000000--uncommitted.md
git add .majordomus/templates && git commit -qm template
before="$(state_hashes .majordomus/state)"
ledger_before="$(cat .majordomus/state/ledger.jsonl)"

# every other command refuses the legacy layout and names the migration; nothing moves
for c in doctor check context "start x --scope lib"; do
  # shellcheck disable=SC2086
  expect_exit 12 "$MJ" $c
  expect_grep 'pre-\.ai layout.*run: majordomus migrate' || { echo "    $c did not name the migration"; exit 1; }
done
[ ! -e .ai ] || { echo "    a refusing command created .ai/"; exit 1; }

# --dry-run prints the whole plan and writes nothing
h0="$(tree_hash)"
expect_exit 0 "$MJ" migrate --dry-run
expect_grep '^  move  \.majordomus/policy\.yaml -> \.ai/repo/policy\.yaml$'
expect_grep '^  move  \.majordomus/profiles/debugging\.yaml -> \.ai/repo/profiles/debugging\.yaml$'
expect_grep '^  saved \.majordomus/providers/body\.md +\(not carried into \.ai'
expect_grep '^  saved \.majordomus/providers/claude-code\.tmpl +\(not carried into \.ai'
expect_grep '^  saved backup first: tmp/majordomus-migrate-backup/<utc>/providers/'
expect_grep '^  move  \.majordomus/templates/handover\.md -> \.ai/repo/templates/handover\.md$'
expect_grep '^  drop  \.majordomus/templates/decisions\.md +\(identical'
expect_grep '^  drop  \.majordomus/generated/fingerprints\.yaml +\(derived'
expect_grep '^  state \.majordomus/state/current\.yaml -> \.ai/local/state/current\.yaml$'
expect_grep '^  state \.majordomus/state/ledger\.jsonl -> \.ai/local/state/ledger\.jsonl$'
expect_grep '^dry run: nothing written$'
[ "$(tree_hash)" = "$h0" ] || { echo "    --dry-run changed the tree"; exit 1; }
[ ! -e .ai ] && [ ! -e tmp ]

# the migration itself
expect_exit 0 "$MJ" migrate
expect_grep '^backup \.majordomus/state/ -> tmp/majordomus-migrate-backup/[0-9TZ]+/state/ \(byte for byte; verified\)$'
expect_grep '^backup \.majordomus/providers/ -> tmp/majordomus-migrate-backup/[0-9TZ]+/providers/ \(byte for byte; verified\)$'
expect_grep '^INFO  providers +\.majordomus/providers/ — 5 file\(s\) copied to tmp/majordomus-migrate-backup/[0-9TZ]+/providers/ and removed, not carried into \.ai: .*\.ai/repo/rules/project/ as rule objects \(docs/DOCTRINE\.md\)  \[reproduce: ls '
expect_grep '^moved [0-9]+ canonical file\(s\) into \.ai/repo/, [0-9]+ state file\(s\) into \.ai/local/state/, dropped [0-9]+ derived file\(s\), backed up and removed 5 provider file\(s\)$'
expect_grep '^removed \.majordomus/ \(empty\)$'
expect_grep '^update: exit 0$'
expect_grep '^doctor: exit 10$'
expect_grep '^migrated: '
backup="$(printf '%s\n' "$LAST_OUT" | sed -n 's/^backup .* -> \(tmp\/majordomus-migrate-backup\/[0-9TZ]*\)\/state\/.*/\1/p')"
[ -d "$backup/state" ] || { echo "    backup directory $backup/state is absent"; exit 1; }
[ ! -e .majordomus ] || { echo "    .majordomus/ survived a complete migration"; exit 1; }

# --- no data loss: the backup is byte for byte; the moved state differs only by what the
# migration itself appended to the ledger
[ "$(state_hashes "$backup/state")" = "$before" ] || { echo "    the backup differs from the state before migration"; exit 1; }
after="$(state_hashes .ai/local/state)"
[ "$(printf '%s\n' "$before" | grep -v ' ./ledger.jsonl$')" = "$(printf '%s\n' "$after" | grep -v ' ./ledger.jsonl$')" ] \
  || { echo "    a state file changed on the way to .ai/local/state"; diff <(printf '%s\n' "$before") <(printf '%s\n' "$after"); exit 1; }
[ "$(head -n "$(printf '%s\n' "$ledger_before" | wc -l | tr -d ' ')" .ai/local/state/ledger.jsonl)" = "$ledger_before" ] \
  || { echo "    the ledger lost or changed a line"; exit 1; }
expect_grep '"event":"layout.migrated"' .ai/local/state/ledger.jsonl
expect_grep '"event":"projections.updated"' .ai/local/state/ledger.jsonl
[ -f .ai/local/state/handovers/99991231T000000Z--master--0000000--uncommitted.md ]

# --- the canonical files moved with their history; the state left the index
git diff --cached --name-status | grep -qE '^R[0-9]*\s+\.majordomus/policy\.yaml\s+\.ai/repo/policy\.yaml$' \
  || { echo "    policy.yaml was not git-moved"; git diff --cached --name-status; exit 1; }
git diff --cached --name-status | grep -qE '^R[0-9]*\s+\.majordomus/templates/handover\.md\s+\.ai/repo/templates/handover\.md$'
grep -q 'custom line' .ai/repo/templates/handover.md
[ ! -e .ai/repo/templates/decisions.md ]
[ -z "$(git ls-files .ai/local)" ] || { echo "    local state is still tracked"; exit 1; }
[ -z "$(git ls-files .majordomus)" ] || { echo "    the index still holds .majordomus/"; exit 1; }
[ "$(grep -c '^\.ai/local/$' .gitignore)" = 1 ]
git check-ignore -q .ai/local/state/current.yaml
git check-ignore -q "$backup/state/ledger.jsonl" || expect_grep 'WARN  backup'

# --- the rest of the layer was seeded, and nothing that moved was overwritten
for f in .ai/README.md .ai/manifest.yaml .ai/repo/README.md .ai/repo/rules/README.md \
         .ai/repo/rules/vendor/majordomus/manifest.yaml .ai/repo/knowledge/sources.yaml \
         .ai/repo/workflows/task-lifecycle.md .ai/repo/profiles/deep-work.yaml; do
  [ -f "$f" ] || { echo "    $f is absent after migration"; exit 1; }
done
expect_grep '^seeded' || true
diff -r "$ROOT/share/standard/majordomus" .ai/repo/rules/vendor/majordomus >/dev/null
# the projections were re-stamped from the new policy path
grep -q 'from \.ai/repo/policy\.yaml' CLAUDE.md
# --- the provider body and its monolithic templates are in the backup, byte for byte, and
# nowhere under .ai; update rendered the tool's thin bootstraps, with no token left behind
for p in body.md claude-code.tmpl codex.tmpl gemini.tmpl generic.tmpl; do
  [ -f "$backup/providers/$p" ] || { echo "    $p is missing from the backup"; exit 1; }
done
[ "$(cat "$backup/providers/body.md")" = "$(printf '## How AI work runs here\n\nTen rules, once monolithic.\n')" ]
[ ! -e .ai/repo/providers ] || { echo "    legacy providers were carried into .ai/repo/providers"; exit 1; }
[ -z "$(git ls-files .majordomus/providers)" ]
expect_no_grep '{{BODY}}' CLAUDE.md
expect_no_grep 'once monolithic' CLAUDE.md
grep -q 'Claude Code bootstrap' CLAUDE.md || { echo "    CLAUDE.md is not the thin bootstrap after migration"; exit 1; }

# --- doctor afterwards fails on hook wiring alone; everything else is real
expect_exit 10 "$MJ" doctor
[ -z "$(printf '%s\n' "$LAST_OUT" | grep '^FAIL' | grep -v ' wiring ')" ] \
  || { echo "    doctor fails on more than wiring after migration:"; printf '%s\n' "$LAST_OUT" | grep '^FAIL'; exit 1; }
expect_grep 'OK   layout +\.ai/ — every section the manifest names exists'
# the records moved intact: the task is still the one the old layout had
expect_exit 0 "$MJ" context
expect_grep 'old task'

# --- idempotent: a second run says so, writes nothing, exits 0
h1="$(tree_hash)"
expect_exit 0 "$MJ" migrate
expect_grep '^already migrated'
[ "$(tree_hash)" = "$h1" ] || { echo "    a second migrate changed the tree"; exit 1; }
git add -A >/dev/null; git commit -qm migrated

# ---------------------------------------------------------------- a tool installation is not project data
fresh_repo
mkdir -p .majordomus/bin && printf '#!/bin/sh\n' > .majordomus/bin/majordomus
expect_exit 12 "$MJ" migrate
expect_grep 'nothing to migrate.*run: majordomus init'
"$MJ" init >/dev/null
expect_exit 0 "$MJ" migrate
expect_grep '^already migrated'
expect_grep 'tool installation'
[ -f .majordomus/bin/majordomus ]

# ---------------------------------------------------------------- mixed: refused, nothing touched
fresh_repo; make_legacy
mkdir -p .majordomus/bin && printf '#!/bin/sh\n' > .majordomus/bin/majordomus
h2="$(tree_hash)"
expect_exit 15 "$MJ" migrate
expect_grep 'both project data \(policy\.yaml\) and a tool installation \(bin/majordomus\)'
expect_grep 'mv \.majordomus/bin'
[ "$(tree_hash)" = "$h2" ] && [ ! -e .ai ] && [ ! -e tmp ]
# a destination that already exists is a refusal too, with the file named
rm -r .majordomus/bin
mkdir -p .ai/repo && printf 'version: 1\n' > .ai/repo/policy.yaml
h3="$(tree_hash)"
expect_exit 15 "$MJ" migrate
expect_grep 'REFUSE conflict +\.ai/repo/policy\.yaml exists; policy\.yaml would overwrite it'
[ "$(tree_hash)" = "$h3" ] && [ ! -e tmp ]
# a legacy policy this version cannot read is refused before anything moves
rm -r .ai
printf 'version: 2\n' > .majordomus/policy.yaml
expect_exit 10 "$MJ" migrate
expect_grep "version '2' \(want 1\)"
[ ! -e .ai ] && [ ! -e tmp ] && [ -d .majordomus/state ]

# ---------------------------------------------------------------- unknown files are preserved
fresh_repo; make_legacy
mkdir -p .majordomus/notes && printf 'keep me\n' > .majordomus/notes/todo.txt
git add -A && git commit -qm notes
expect_exit 0 "$MJ" migrate --dry-run
expect_grep '^  keep  \.majordomus/notes/todo\.txt'
expect_grep 'WARN  unknown +\.majordomus/ — 1 file\(s\)'
expect_exit 0 "$MJ" migrate
expect_grep 'WARN  unknown +\.majordomus/ — remains'
expect_grep '^  \.majordomus/notes/todo\.txt$'
[ "$(cat .majordomus/notes/todo.txt)" = "keep me" ]
[ ! -e .majordomus/policy.yaml ] && [ ! -d .majordomus/state ] && [ -f .ai/repo/policy.yaml ]
git ls-files --error-unmatch .majordomus/notes/todo.txt >/dev/null
# still idempotent with the leftover present
expect_exit 0 "$MJ" migrate
expect_grep '^already migrated'
expect_grep 'remains with 1 file'
