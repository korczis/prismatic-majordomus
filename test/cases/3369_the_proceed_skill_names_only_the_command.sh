# majordomus-covers: skills
# majordomus-skill: proceed
# /proceed is a generated skill that names only the command (I2975, ADR 0177 decision 4).
#
# The decision and the take belong to `majordomus proceed plan` and `majordomus proceed
# take`; the skill only says to run them and what to do with each answer. A skill that
# restated a rule, a workflow step or an issue would be a second copy of something the
# repository already says, and would go stale the first time that changed. So this case
# judges the canonical file the way a reviewer would, and every judgement is mechanical:
#
#   1. the body names no issue id, no rule id, no ADR and no workflow file;
#   2. every `majordomus ...` it names is a command of the shell tool or a path of the Rust
#      executable, read from the generated catalogues rather than from a list kept here;
#   3. every skill it invokes as `majordomus://skill/<id>` exists;
#   4. a projection exists for every provider that loads skills, each generated from this one
#      file and saying nothing of its own;
#   5. each way of restating something is refused when it is planted in a copy, so the
#      judgement above cannot pass by finding nothing to look at.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq not installed"
SKILL="$ROOT/.ai/repo/skills/proceed/SKILL.md"
CLI="$ROOT/docs/generated/cli.json"
SHELL_COMMANDS="$ROOT/site/data/generated/commands.json"
expect_file "$SKILL"
expect_file "$CLI"
expect_file "$SHELL_COMMANDS"

jq -r '.. | objects | select(has("path") and has("usage")) | .path[1:] | join(" ")' "$CLI" \
  | sed '/^$/d' > "$T/rust.txt"
jq -r '.commands[].name' "$SHELL_COMMANDS" > "$T/shell.txt"
[ -s "$T/rust.txt" ] && [ -s "$T/shell.txt" ] || { echo "    a command catalogue read empty"; exit 1; }
grep -qx 'proceed plan' "$T/rust.txt" || { echo "    the Rust executable declares no proceed plan"; exit 1; }
grep -qx 'proceed take' "$T/rust.txt" || { echo "    the Rust executable declares no proceed take"; exit 1; }

# judge <file>: prints one line per finding, nothing for a thin skill
judge() {
  local body="$T/body.md" cmd first
  awk 'f; /^---$/ && ++n == 2 { f = 1 }' "$1" > "$body"
  { grep -noE '\bI[0-9]{4,}\b' "$body" || true; } | sed 's/^/issue /'
  { grep -noE '\b(project|majordomus)\.[a-z0-9]+(-[a-z0-9]+)+\b' "$body" || true; } | sed 's/^/rule /'
  { grep -noE '\bADR ?[0-9]{2,}\b' "$body" || true; } | sed 's/^/adr /'
  { grep -noE '\.ai/repo/(workflows|rules)/[^ `)]*' "$body" || true; } | sed 's/^/workflow /'
  { grep -oE '`majordomus [a-z][a-z -]*[a-z]`' "$body" || true; } | tr -d '`' | sed 's/^majordomus //' \
    | LC_ALL=C sort -u | while read -r cmd; do
      first="${cmd%% *}"
      # a path of the Rust executable is named whole; the shell tool's subcommands are its own
      grep -qx "$cmd" "$T/rust.txt" || grep -qx "$first" "$T/shell.txt" \
        || printf 'command %s\n' "$cmd"
    done
  { grep -oE 'majordomus://skill/[a-z0-9-]+' "$body" || true; } | LC_ALL=C sort -u | while read -r uri; do
    [ -f "$ROOT/.ai/repo/skills/${uri##*/}/SKILL.md" ] || printf 'skill %s\n' "$uri"
  done
  return 0
}

# ---------------------------------------------------------------- 1-3. the canonical file
judge "$SKILL" > "$T/findings.txt"
[ ! -s "$T/findings.txt" ] || { echo "    the proceed skill restates what it should name:"; cat "$T/findings.txt"; exit 1; }
grep -q '`majordomus proceed plan`' "$SKILL" || { echo "    the skill does not run proceed plan"; exit 1; }
grep -q '`majordomus proceed take`' "$SKILL" || { echo "    the skill does not run proceed take"; exit 1; }
grep -q 'majordomus://skill/implement' "$SKILL" || { echo "    the skill does not hand the work to implement"; exit 1; }

# ---------------------------------------------------------------- 4. one declaration, every provider
for dir in .claude/skills .agents/skills; do
  p="$ROOT/$dir/proceed/SKILL.md"
  expect_file "$p"
  grep -qx 'name: proceed' "$p" || { echo "    $dir/proceed is not named proceed"; exit 1; }
  grep -q 'GENERATED FILE' "$p" || { echo "    $dir/proceed is not generated"; exit 1; }
  grep -q 'Source: .ai/repo/skills/proceed/SKILL.md' "$p" || { echo "    $dir/proceed names another source"; exit 1; }
  ! grep -q '^# Procedure' "$p" || { echo "    $dir/proceed carries a procedure of its own"; exit 1; }
done

# ---------------------------------------------------------------- 5. each restatement is refused
plant() { # <kind> <line>: the skill with <line> added to its procedure is refused as <kind>
  awk -v l="$2" '{ print } /^# Procedure$/ { print ""; print l }' "$SKILL" > "$T/planted.md"
  judge "$T/planted.md" > "$T/planted.txt"
  grep -q "^$1 " "$T/planted.txt" || { echo "    a planted $1 ($2) was not refused:"; cat "$T/planted.txt"; exit 1; }
}
plant issue 'Take I2974 first.'
plant rule 'Keep project.worktree-topology in mind.'
plant adr 'As ADR 0167 says, never land.'
plant workflow 'Then follow .ai/repo/workflows/task-lifecycle.md step by step.'
plant command 'Run `majordomus proceed everything` when in doubt.'
plant skill 'Follow majordomus://skill/no-such-skill.'
exit 0
