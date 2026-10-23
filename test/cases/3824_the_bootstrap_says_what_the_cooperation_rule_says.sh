# The bootstrap says what the cooperation rule says (I3824, ADR 0167 amendment of 2026-10-20).
#
# Every session reads the "Work moves through seats" paragraph of its provider's instruction
# file before it reads any rule. Version 2 of rule project.the-cooperation-model-is-the-owners-word
# lets a worker land its own green head through the guarded lander's queue and bounds builds by
# the machine (five builders, 35 GiB). A bootstrap still saying "never merges" would tell every
# new session the opposite of the owner's word, so this case holds:
#
#   1  the rule is version 2 and states the queue, the two limits and the cleanup clause;
#   2  the three provider templates carry one identical paragraph, and each bootstrap generated
#      from them carries it too;
#   3  that paragraph says a worker lands its own green head through the guarded queue, names
#      both limits and the rule, and never says "never merges" — proven by mutation;
#   4  no tracked source file links the version 1 rule file.
#
# Read from the tracked files themselves; nothing is built and no network is reached.
. "$ROOT/test/lib.sh"
fail=0
note() { printf '    %s\n' "$1"; fail=1; }

RULE="$ROOT/.ai/repo/rules/project/the-cooperation-model-is-the-owners-word.v2.md"
[ -f "$RULE" ] || { echo "    ${RULE#"$ROOT"/} is missing"; exit 1; }

# ---------------------------------------------------------------- 1. the rule
grep -q '^version: 2$' "$RULE" || note "the rule does not say version: 2"
for phrase in "guarded lander's queue" "five builders" "35 GiB" "without asking" "2026-10-20"; do
  grep -qF "$phrase" "$RULE" || note "the rule does not state \"$phrase\""
done

# ---------------------------------------------------------------- 2-3. the paragraph
# The paragraph that opens with "Work moves through seats", up to the next blank line.
paragraph() { awk '/^Work moves through seats/{p=1} p&&/^$/{exit} p' "$1"; }

# Judge one paragraph file; prints each defect and returns 1 when there is one.
judge() {
  local text="$1" bad=0
  [ -n "$text" ] || { echo "no 'Work moves through seats' paragraph"; return 1; }
  local flat
  flat="$(printf '%s' "$text" | tr '\n' ' ')"
  case "$flat" in *"never merges"*) echo "says 'never merges'"; bad=1 ;; esac
  for phrase in "lands its own" "guarded lander's queue" "five builders" "35 GiB" \
    "project.the-cooperation-model-is-the-owners-word"; do
    case "$flat" in *"$phrase"*) ;; *) echo "does not say \"$phrase\""; bad=1 ;; esac
  done
  return "$bad"
}

reference=""
for t in claude-code agents gemini; do
  tmpl="$ROOT/.ai/repo/providers/$t.tmpl"
  [ -f "$tmpl" ] || { note ".ai/repo/providers/$t.tmpl is missing"; continue; }
  text="$(paragraph "$tmpl")"
  if ! out="$(judge "$text")"; then
    while IFS= read -r line; do note ".ai/repo/providers/$t.tmpl $line"; done <<EOF_OUT
$out
EOF_OUT
  fi
  if [ -z "$reference" ]; then
    reference="$text"
  elif [ "$text" != "$reference" ]; then
    note ".ai/repo/providers/$t.tmpl carries a different paragraph from claude-code.tmpl"
  fi
done

for f in CLAUDE.md AGENTS.md GEMINI.md; do
  [ -f "$ROOT/$f" ] || { note "$f is missing"; continue; }
  [ "$(paragraph "$ROOT/$f")" = "$reference" ] \
    || note "$f does not carry the templates' paragraph: run majordomus update"
done

# The judge refuses what it must: the same paragraph with the old words put back.
mutated="$(printf '%s' "$reference" | sed "s/lands its own/hands its/; s/guarded lander's queue/landing seat and never merges/")"
judge "$mutated" >/dev/null && note "the judge passed a paragraph that says 'never merges'"

# ---------------------------------------------------------------- 4. no link to version 1
stale="$(cd "$ROOT" && git grep -l 'the-cooperation-model-is-the-owners-word\.v1\.md' -- . \
  ':!site/i18n' ':!test/cases/3824_the_bootstrap_says_what_the_cooperation_rule_says.sh' 2>/dev/null || true)"
[ -z "$stale" ] || note "these files still link the version 1 rule: $(echo $stale)"

[ "$fail" -eq 0 ] || exit 1
echo "    the rule is version 2, and every bootstrap says a worker lands its own green head"
