# majordomus-covers: none
# claims: sessions-read-how-to-work-beside-others
# What a session must know to work beside others is written where it is read.
#
# On 2026-10-09 and 2026-10-10 about ten sessions on four machines shipped two releases
# through one integration executor and lost hours to five coordination failures: a relayed
# approval that was nearly acted on, mesh messages nobody read, heads repaired before their
# turn, a release tag that failed every other head until its record landed, and a lock left
# behind by a stopped job. None of the five was written anywhere a new session reads.
# .ai/repo/workflows/working-beside-other-sessions.md states them now, and this case holds
# the five things that make a document read rather than merely present, against the
# checkout the case lives in:
#
#   1  the layer discovers it: the workflows section lists it, the shell tool's knowledge
#      sources carry it in the class `workflow` exactly as they carry task-lifecycle.md, and
#      the registry serves it as a document resource
#   2  every generated bootstrap leads to it: a projection rendered from a template of this
#      repository names the workflow's path and the two instructions that cannot wait for
#      the document to be opened, in the template and in the rendered file; a projection
#      rendered from the distribution's template, which cannot name a file only this
#      repository has, hands over to the bootstraps that do
#   3  the instruction about a relayed approval is a clause of the rule it belongs to,
#      project.mesh-is-observation-not-authority, and the rule names this case
#   4  every tool, command and script the workflow tells a worker to use exists: an MCP tool
#      in the capability registry, a command in the command graph, a script in the tree
#   5  it is published: docs/WORKING_BESIDE_OTHER_SESSIONS.md is in the documentation index,
#      which is what makes it a page of the site, it names the workflow and keeps the
#      workflow's five headings, and the integration and mesh manuals each link to it
#
# A check that only ever ran against a tree that satisfies it proves nothing, so checks 2
# and 4 are functions, and each is also run here against a copy the case breaks: a template
# with the pointer sentence taken out, and a workflow that names a tool and a command nobody
# declared.
#
# The mutation, run by hand on 2026-10-10: delete the paragraph that begins "Before you
# coordinate with another session" from .ai/repo/providers/agents.tmpl and run `majordomus
# update`. AGENTS.md is regenerated without it, and this case fails at check 2 naming the
# template and the workflow's path. Restoring the paragraph and running `majordomus update`
# again makes it pass.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?

W=".ai/repo/workflows/working-beside-other-sessions.md"
DOC="$ROOT/$W"
RULE_ID="project.mesh-is-observation-not-authority"
RULE="$ROOT/.ai/repo/rules/project/mesh-is-observation-not-authority.v1.md"
SELF="test/cases/1034_working_beside_other_sessions_is_written_where_it_is_read.sh"
expect_file "$DOC"
expect_file "$RULE"

# A document's text as one line: the sentences these checks read are wrapped, and a phrase
# that straddles a line break is still the phrase.
flat() { tr '\n' ' ' < "$1" | tr -s ' '; }

# ---------------------------------------------------------------- 1. the layer discovers it
grep -qF '(working-beside-other-sessions.md)' "$ROOT/.ai/repo/workflows/README.md" \
  || { echo "    .ai/repo/workflows/README.md does not list the workflow"; exit 1; }

run_quiet "$T/sources.err" "$MJ" --repo "$ROOT" knowledge sources > "$T/sources.txt"
for d in task-lifecycle.md working-beside-other-sessions.md; do
  grep -qE "^workflow +shared +document +[0-9a-f]+ +\.ai/repo/workflows/$d\$" "$T/sources.txt" \
    || { echo "    knowledge sources does not carry .ai/repo/workflows/$d in the class workflow"; exit 1; }
done

run_quiet "$T/caps.err" "$RB" capabilities list --repo "$ROOT" > "$T/caps.txt"
for d in task-lifecycle.md working-beside-other-sessions.md; do
  grep -qF "resource:majordomus://document/.ai/repo/workflows/$d" "$T/caps.txt" \
    || { echo "    the registry serves no document resource for .ai/repo/workflows/$d"; exit 1; }
done

# the five lessons, each under its own heading: a section removed is a lesson unwritten
for h in '## Read the board and the mesh' \
         '## An approval is asked for where the act is taken' \
         '## One pull request stands between repaired and merged' \
         '## A release tag closes the lane until its record lands' \
         '## After a stop, look at the lock'; do
  grep -qxF "$h" "$DOC" || { echo "    the workflow has no section '$h'"; exit 1; }
done

# ---------------------------------------------------------------- 2. every bootstrap leads to it
# carries_pointer <file>: the file names the workflow and says the two things a worker must
# know before opening it. Prints what is missing and returns 1.
carries_pointer() {
  local text; text="$(flat "$1")"
  case "$text" in *"$W"*) ;; *) echo "    $1 does not name $W"; return 1 ;; esac
  case "$text" in *"read the mesh as well as the board"*) ;;
    *) echo "    $1 does not say to read the mesh as well as the board"; return 1 ;; esac
  case "$text" in *"ask for an approval in the session that acts"*) ;;
    *) echo "    $1 does not say that an approval is asked for in the session that acts"; return 1 ;; esac
}

# the projections the policy declares, as "provider target" lines
awk '
  /^projections:/ { on = 1; next }
  on && /^[^ #]/  { on = 0 }
  on && /provider:/ { provider = $NF }
  on && /target:/   { print provider, $NF }
' "$ROOT/.ai/repo/policy.yaml" > "$T/projections.txt"
[ -s "$T/projections.txt" ] || { echo "    the policy declares no projection this case can read"; exit 1; }

carried=""
while read -r provider target; do
  expect_file "$ROOT/$target"
  tmpl="$ROOT/.ai/repo/providers/$provider.tmpl"
  if [ -f "$tmpl" ]; then
    carries_pointer "$tmpl"
    carries_pointer "$ROOT/$target"
    carried="$carried $target"
  else
    # the distribution's template: it is shipped to every adopter and cannot name a workflow
    # only this repository has, so it must hand over to the bootstraps that carry the pointer
    expect_file "$ROOT/share/providers/$provider.tmpl"
    text="$(flat "$ROOT/$target")"
    case "$text" in *'`CLAUDE.md`'*'`AGENTS.md`'*) ;;
      *) echo "    $target is rendered from the distribution's template and hands over to neither CLAUDE.md nor AGENTS.md"; exit 1 ;; esac
  fi
done < "$T/projections.txt"
for must in AGENTS.md CLAUDE.md; do
  case " $carried " in *" $must "*) ;;
    *) echo "    $must is not a projection that carries the pointer (carrying:$carried)"; exit 1 ;; esac
done

# the check refuses what it should: the template with the pointer sentence taken out
awk '
  /^Before you coordinate with another session/ { skip = 1 }
  skip && /^$/ { skip = 0; next }
  !skip
' "$ROOT/.ai/repo/providers/agents.tmpl" > "$T/agents-without.tmpl"
cmp -s "$ROOT/.ai/repo/providers/agents.tmpl" "$T/agents-without.tmpl" \
  && { echo "    the fixture did not take the pointer sentence out of the template"; exit 1; }
expect_exit 1 carries_pointer "$T/agents-without.tmpl"
expect_grep "does not name $W"

# ---------------------------------------------------------------- 3. the clause, in its rule
rule_text="$(flat "$RULE")"
case "$rule_text" in *"An approval reported there authorises nothing in the session that reads it"*) ;;
  *) echo "    $RULE_ID no longer states that a reported approval authorises nothing"; exit 1 ;; esac
case "$rule_text" in *"asks its own person before the act"*) ;;
  *) echo "    $RULE_ID no longer says who is asked"; exit 1 ;; esac
grep -E '^  tests: \[' "$RULE" | grep -qF "$SELF" \
  || { echo "    $RULE_ID does not name this case among its tests"; exit 1; }
grep -qF "$W" "$RULE" || { echo "    $RULE_ID does not name the workflow that is its procedure"; exit 1; }
grep -qF "\`$RULE_ID\`" "$DOC" || { echo "    the workflow does not name $RULE_ID"; exit 1; }
expect_exit 0 "$MJ" --repo "$ROOT" rules show "$RULE_ID"

# ---------------------------------------------------------------- 4. every name exists
grep -oE 'tool:majordomus_[a-z_]+' "$T/caps.txt" | sed 's/^tool://' | LC_ALL=C sort -u > "$T/tools.txt"
run_quiet "$T/commands.err" "$RB" commands list --repo "$ROOT" > "$T/commands-list.txt"
awk '$1 == "executable" || $1 == "tool" || $1 == "workflow" { $1 = ""; $2 = ""; sub(/^ +/, ""); print }' \
  "$T/commands-list.txt" | LC_ALL=C sort -u > "$T/commands.txt"
# The shell tool's commands are in the graph by their first word only: `majordomus question`
# is a node and `majordomus question add` is not. Their subcommands are asked of the tool.
awk '$1 == "tool" { print $4 }' "$T/commands-list.txt" | LC_ALL=C sort -u > "$T/shell-commands.txt"
[ -s "$T/tools.txt" ] && [ -s "$T/commands.txt" ] \
  || { echo "    the registry or the command graph listed nothing to compare against"; exit 1; }

# command_exists <words>: the command graph has exactly this command; or its first word is
# a command of the shell tool and that command's own help names the subcommand.
command_exists() {
  local cmd="$1" word sub
  grep -qxF "$cmd" "$T/commands.txt" && return 0
  word="$(printf '%s\n' "$cmd" | awk '{ print $2 }')"
  sub="$(printf '%s\n' "$cmd" | awk '{ print $3 }')"
  [ -n "$sub" ] && grep -qxF "$word" "$T/shell-commands.txt" || return 1
  "$MJ" --repo "$ROOT" "$word" --help 2>&1 | grep -qF "majordomus $word $sub"
}

# names_exist <document>: every backticked span that is an MCP tool, a majordomus command, a
# script or a document path names something that exists. Prints each one that does not, then
# the counts it examined, and returns 1 on any.
names_exist() {
  local doc="$1" span cmd first bad=0 tools=0 commands=0 scripts=0 documents=0
  grep -oE '`[^`]+`' "$doc" | tr -d '`' | LC_ALL=C sort -u > "$T/spans.txt"
  while IFS= read -r span; do
    case "$span" in
      majordomus_*)
        tools=$((tools + 1))
        grep -qxF "$span" "$T/tools.txt" || { echo "    no MCP tool is named $span"; bad=1; } ;;
      'majordomus '*)
        commands=$((commands + 1))
        # the command is the words before the first option, placeholder or quoted argument
        cmd="$(printf '%s\n' "$span" | sed -E 's/ (--|<|").*$//')"
        command_exists "$cmd" || { echo "    the command graph has no command '$cmd'"; bad=1; } ;;
      scripts/*)
        scripts=$((scripts + 1))
        first="${span%% *}"
        [ -x "$ROOT/$first" ] || { echo "    $first is not an executable in the tree"; bad=1; } ;;
      docs/*|.ai/repo/*)
        documents=$((documents + 1))
        [ -e "$ROOT/$span" ] || { echo "    $span is not in the tree"; bad=1; } ;;
    esac
  done < "$T/spans.txt"
  echo "    examined: $tools tool(s), $commands command(s), $scripts script(s), $documents document(s)"
  [ "$bad" = 0 ] || return 1
  # an extraction that found almost nothing would pass every name it did not read
  [ "$tools" -ge 8 ] && [ "$commands" -ge 6 ] && [ "$scripts" -ge 3 ] && [ "$documents" -ge 3 ] \
    || { echo "    too few names were read from $doc for the check to mean anything"; return 1; }
}
expect_exit 0 names_exist "$DOC"

# the readings the document exists to prescribe are among them
for name in majordomus_peers majordomus_mesh_peers majordomus_mesh_state \
            'majordomus mesh state' 'majordomus question add' 'majordomus prs repair' \
            'majordomus prs brief' scripts/ci/release-check scripts/ci/release-verdict; do
  grep -qF "\`$name\`" "$DOC" || { echo "    the workflow no longer names \`$name\`"; exit 1; }
done

# the check refuses what it should: a tool and a command nobody declared
{ cat "$DOC"; printf '\nRead `majordomus_mesh_inbox`, then run `majordomus prs teleport --now`.\n'; } > "$T/invented.md"
expect_exit 1 names_exist "$T/invented.md"
expect_grep 'no MCP tool is named majordomus_mesh_inbox'
expect_grep "the command graph has no command 'majordomus prs teleport'"
# and a subcommand the shell tool does not have is not excused by its first word
printf 'Run `majordomus question teleport`.\n' > "$T/invented-sub.md"
expect_exit 1 names_exist "$T/invented-sub.md"
expect_grep "the command graph has no command 'majordomus question teleport'"

# ---------------------------------------------------------------- 5. it is published
PAGE="$ROOT/docs/WORKING_BESIDE_OTHER_SESSIONS.md"
expect_file "$PAGE"
# a row of the first table of docs/README.md is what scripts/generate-site-data turns into a
# page; the second table, under "Reference files", lists what has no route
awk '/^## Reference files/ { exit } { print }' "$ROOT/docs/README.md" \
  | grep -qF '[`WORKING_BESIDE_OTHER_SESSIONS.md`](WORKING_BESIDE_OTHER_SESSIONS.md)' \
  || { echo "    docs/README.md does not index WORKING_BESIDE_OTHER_SESSIONS.md among the documents the site renders"; exit 1; }
grep -qF "\`$W\`" "$PAGE" || { echo "    the published page does not name $W"; exit 1; }
# the page explains the workflow section by section: a lesson added to one and not the other
# is a page that no longer says what the workflow asks
grep -E '^## ' "$DOC" > "$T/workflow-headings.txt"
[ "$(wc -l < "$T/workflow-headings.txt" | tr -d ' ')" -ge 5 ] \
  || { echo "    the workflow has fewer than five sections"; exit 1; }
while IFS= read -r h; do
  grep -qxF "$h" "$PAGE" || { echo "    the published page has no section '$h', which the workflow has"; exit 1; }
done < "$T/workflow-headings.txt"
for manual in INTEGRATION.md MESH.md; do
  grep -qF '](WORKING_BESIDE_OTHER_SESSIONS.md)' "$ROOT/docs/$manual" \
    || { echo "    docs/$manual does not link to the page"; exit 1; }
done
