# majordomus-covers: adr prompt
# majordomus-negative: adr
# majordomus-skill: promote-a-workspace
# The skill `promote-a-workspace` is a procedure over commands the repository already has,
# and the two briefs it hands its workers are prompt assets the tool renders, not text kept
# beside it. The store it fills is the git-ignored one the workspace contract names, and no
# step of it sends a conversation's content anywhere under .ai/repo/.
#
# What is proved: the skill is invoked by the task lifecycle and by both prompts, and hands
# its workers each brief through `prompt render`, the verb that substitutes the tokens; each
# `majordomus <group> <subcommand>` its text names, for the groups the shell tool owns, is
# one the tool lists; each script it names exists; each capability it runs is one the
# registry declares; each Cockpit object route it names uses a kind share/kinds.yaml
# declares; each prompt it names is a file that renders with every token substituted and
# keeps its privacy rules; it names the store under .ai/local/workspaces/ and no other place
# for raw content, and that store is git-ignored; a decision proposed the way step 7 says is
# written proposed and extracted, and `adr check` refuses it once it claims to be accepted;
# the skill is reachable by its name in both provider projections; and each command of the
# Rust executable it names is one that executable answers.
. "$ROOT/test/lib.sh"

SKILL="$ROOT/.ai/repo/skills/promote-a-workspace/SKILL.md"
PROMPTS="workspace-extract workspace-consolidate"
[ -f "$SKILL" ] || { echo "    .ai/repo/skills/promote-a-workspace/SKILL.md is missing"; exit 1; }

# ---------------------------------------------------------------- it is invoked
grep -q 'majordomus://skill/promote-a-workspace' "$ROOT/.ai/repo/workflows/task-lifecycle.md" \
  || { echo "    the task lifecycle does not invoke majordomus://skill/promote-a-workspace"; exit 1; }
for p in $PROMPTS; do
  grep -q "prompt render $p" "$SKILL" || { echo "    the skill does not hand its workers the prompt $p rendered"; exit 1; }
  [ -f "$ROOT/.ai/repo/prompts/$p.md" ] || { echo "    the skill names the prompt $p, which does not exist"; exit 1; }
  grep -q 'majordomus://skill/promote-a-workspace' "$ROOT/.ai/repo/prompts/$p.md" \
    || { echo "    the prompt $p does not say which skill it is a step of"; exit 1; }
done

# ---------------------------------------------------------------- it names what exists
for grp in knowledge adr plan prompt; do
  help="$("$MJ" "$grp" --help 2>&1 || true)"
  for sub in $(grep -oE "majordomus $grp [a-z]+" "$SKILL" | awk '{ print $3 }' | LC_ALL=C sort -u); do
    printf '%s\n' "$help" | grep -qE "^  $sub( |\$)|majordomus $grp $sub( |\$)" \
      || { echo "    the skill names 'majordomus $grp $sub', which the tool does not list"; exit 1; }
  done
done
for sub in validate show; do
  grep -q "majordomus plan $sub" "$SKILL" || { echo "    the procedure does not use 'majordomus plan $sub'"; exit 1; }
done
for sub in next propose check; do
  grep -q "majordomus adr $sub" "$SKILL" || { echo "    the procedure does not use 'majordomus adr $sub'"; exit 1; }
done
for s in $(grep -oE 'scripts/[a-z/-]+[a-z]' "$SKILL" | LC_ALL=C sort -u); do
  [ -e "$ROOT/$s" ] || { echo "    the skill names $s, which does not exist"; exit 1; }
done
for cap in $(grep -oE 'majordomus-cli run [a-z_.]+' "$SKILL" | awk '{ print $3 }' | LC_ALL=C sort -u); do
  grep -rqF "\"$cap\"" "$ROOT/apps/majordomus-cli/src/capability/builtin" \
    || { echo "    the skill runs the capability $cap, which the registry does not declare"; exit 1; }
done
for k in $(grep -oE '/cockpit/objects/[a-z-]+/' "$SKILL" | awk -F/ '{ print $4 }' | LC_ALL=C sort -u); do
  grep -qE "^  $k:\$" "$ROOT/share/kinds.yaml" \
    || { echo "    the skill names /cockpit/objects/$k/, and $k is not a kind of share/kinds.yaml"; exit 1; }
done

# ---------------------------------------------------------------- the content stays in the store
grep -q '\.ai/local/workspaces/<workspace>/' "$SKILL" \
  || { echo "    the skill does not name the workspace store under .ai/local/workspaces/"; exit 1; }
for f in "$SKILL" "$ROOT/.ai/repo/prompts/workspace-extract.md" "$ROOT/.ai/repo/prompts/workspace-consolidate.md"; do
  if grep -nE '\.ai/repo/[A-Za-z0-9/<>_-]*(raw|corpus|transcript|conversations|export)' "$f"; then
    echo "    ${f#"$ROOT"/} sends workspace content under .ai/repo/"; exit 1
  fi
done
for p in $PROMPTS; do
  grep -qiE 'off-?topic' "$ROOT/.ai/repo/prompts/$p.md" \
    || { echo "    the prompt $p no longer keeps off-topic conversations out"; exit 1; }
  grep -qF '[redacted]' "$ROOT/.ai/repo/prompts/$p.md" \
    || { echo "    the prompt $p no longer redacts credentials and personal data"; exit 1; }
done

# ---------------------------------------------------------------- the briefs render, the decision stays proposed
unset MAJORDOMUS_PROVIDER_SESSION CLAUDE_CODE_SESSION_ID
"$MJ" init >/dev/null; "$MJ" update >/dev/null
git add .gitignore >/dev/null 2>&1; git commit -qm "ignore local ai state" >/dev/null 2>&1 || true
git check-ignore -q .ai/local/workspaces/x/raw/y.json \
  || { echo "    the workspace store .ai/local/workspaces/ is not git-ignored"; exit 1; }
S="$(mktemp -d "${TMPDIR:-/tmp}/mj.promote.XXXXXX")"; trap 'rm -rf "$S"' EXIT
for p in $PROMPTS; do cp "$ROOT/.ai/repo/prompts/$p.md" ".ai/repo/prompts/$p.md"; done
"$MJ" start "promote a workspace" --scope .ai/repo >/dev/null
for p in $PROMPTS; do
  "$MJ" prompt render "$p" >"$S/$p.out" 2>"$S/$p.err" \
    || { echo "    the prompt $p does not render:"; sed 's/^/    | /' "$S/$p.err"; exit 1; }
  if grep -n '{{' "$S/$p.out"; then echo "    the prompt $p left a token unsubstituted"; exit 1; fi
done

"$MJ" adr propose "A workspace record cites the message it came from" --from issue:I0001 \
  >"$S/propose.out" 2>"$S/propose.err" \
  || { echo "    adr propose was refused:"; sed 's/^/    | /' "$S/propose.err"; exit 1; }
ADR="$(find .ai/repo/adrs -name '*-a-workspace-record-cites-*.md' | head -n 1)"
[ -n "$ADR" ] || { echo "    adr propose wrote no decision:"; sed 's/^/    | /' "$S/propose.out"; exit 1; }
grep -q '^status: proposed$' "$ADR" || { echo "    the proposed decision is not proposed"; exit 1; }
grep -q '^  origin: extracted$' "$ADR" || { echo "    a decision proposed --from a reference is not extracted"; exit 1; }
# discovery reads the git index: an untracked record is invisible to the check (step 7)
git add "$ADR"
expect_exit 0 "$MJ" adr check
expect_grep 'examined 1 decision'
sed 's/^status: proposed$/status: accepted/' "$ADR" >"$S/accepted.md" && cp "$S/accepted.md" "$ADR"
git add "$ADR"
expect_exit 10 "$MJ" adr check
expect_grep 'extracted'

# ---------------------------------------------------------------- it is reachable by its name
for P in "$ROOT/.claude/skills/promote-a-workspace/SKILL.md" "$ROOT/.agents/skills/promote-a-workspace/SKILL.md"; do
  [ -f "$P" ] || { echo "    ${P#"$ROOT"/} is missing: the skill is not reachable by its name there"; exit 1; }
  [ "$(sed -n 2p "$P")" = "name: promote-a-workspace" ] || { echo "    ${P#"$ROOT"/} is misnamed: $(sed -n 2p "$P")"; exit 1; }
  grep -q 'GENERATED FILE' "$P" || { echo "    ${P#"$ROOT"/} is not a generated projection"; exit 1; }
  grep -qF '(../../../.ai/repo/skills/promote-a-workspace/SKILL.md)' "$P" \
    || { echo "    ${P#"$ROOT"/} does not lead to the definition"; exit 1; }
done

# ---------------------------------------------------------------- the Rust executable answers what it names
RB="$(rust_bin)" || rust_bin_exit $?
{ grep -oE 'majordomus-cli [a-z]+ [a-z]+' "$SKILL" | awk '$2 != "run" { print $2, $3 }'
  grep -oE 'majordomus (intent|why|worktree) [a-z]+' "$SKILL" | awk '{ print $2, $3 }'
} | LC_ALL=C sort -u | while read -r grp sub; do
  "$RB" "$grp" "$sub" --help >/dev/null 2>&1 \
    || { echo "    the skill names '$grp $sub', which the Rust executable does not answer"; exit 1; }
done || exit 1
