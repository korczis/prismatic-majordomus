# majordomus-covers: none
# The gate of project.providers-are-data, driven against fixture repositories.
#
# A provider is declared once — share/providers.yaml and the template beside it — and every
# other appearance of it is a projection: the generated document, the bootstrap the policy
# writes, the merge attribute that keeps that bootstrap from being resolved by hand. The
# failure this gate exists to stop is the quiet one: a provider added in one of those places
# and not the others, which nothing notices until a merge leaves a bootstrap that describes
# neither side.
#
# `scripts/ci/providers-check` was named by nothing in the suite. Its third check is the one
# that most needed a case: "no document enumerates providers by hand" is a heuristic — a
# line that names three or more declared providers — and a heuristic nobody has watched
# refuse anything is indistinguishable from a heuristic that matches nothing. It is exercised
# here from both sides: the line that must be caught, the same line beside a pointer at the
# projection, and a line naming two providers, which is a sentence rather than a list.
#
# Every fixture is a repository of its own, because the gate reads the tracked document set.
. "$ROOT/test/lib.sh"
GATE="$ROOT/scripts/ci/providers-check"
[ -x "$GATE" ] || { echo "    $GATE is not executable"; exit 1; }
command -v jq >/dev/null 2>&1 || { echo "    skip: no jq"; exit 0; }

# A tree with the shape the gate reads: three providers declared, a template for each, the
# generated document the executable wrote from both, a policy that projects through each,
# and a .gitattributes marking every projected bootstrap merge=derived.
fixture() {   # fixture <n> -> prints the tree
  F="$T/tree$1"
  rm -rf "$F"
  mkdir -p "$F/docs/generated" "$F/share/providers" "$F/.ai/repo"
  cat > "$F/docs/generated/providers.json" <<'JSON'
{
  "schema": "majordomus/providers/v1",
  "providers": [
    { "id": "claude", "title": "Claude Code", "bootstraps": [ { "target": "CLAUDE.md" } ] },
    { "id": "codex",  "title": "Codex CLI",   "bootstraps": [ { "target": ".codex/config.toml" } ] },
    { "id": "gemini", "title": "Gemini CLI",  "bootstraps": [ { "target": ".gemini/settings.json" } ] }
  ]
}
JSON
  printf 'bootstrap for claude\n' > "$F/share/providers/claude.tmpl"
  printf 'bootstrap for codex\n'  > "$F/share/providers/codex.tmpl"
  printf 'bootstrap for gemini\n' > "$F/share/providers/gemini.tmpl"
  cat > "$F/share/providers.yaml" <<'YAML'
providers:
  claude:
    title: Claude Code
  codex:
    title: Codex CLI
  gemini:
    title: Gemini CLI
YAML
  cat > "$F/.ai/repo/policy.yaml" <<'YAML'
projections:
  - provider: claude
  - provider: codex
  - provider: gemini
YAML
  cat > "$F/.gitattributes" <<'ATTR'
CLAUDE.md merge=derived
.codex/config.toml merge=derived
.gemini/settings.json merge=derived
ATTR
  printf '# Fixture\n\nThe tool projects a bootstrap for each declared provider.\n' > "$F/README.md"
  ( cd "$F" && git init -q . && git add -A \
      && git -c user.email=t@example.com -c user.name=t commit -q -m fixture )
  printf '%s' "$F"
}

# The gate reads the tracked document set, so a fixture that adds a document stages it.
gate() {   # gate <tree> [env assignments handled by caller]
  local f="$1"
  ( cd "$f" && git add -A >/dev/null 2>&1 ) || true
  env MJ_ROOT="$f" "$GATE"
}

# ---------------------------------------------------------------- the tree it accepts
F="$(fixture 0)"
expect_exit 0 gate "$F"
expect_grep 'one declaration, every projection current, no provider list written by hand'
expect_grep "provider 'claude' has a template"
expect_grep 'CLAUDE.md is merge=derived'

# ---------------------------------------------------------------- 1. declaration and template
# Both directions, because the defect can arrive from either end: a provider declared with
# nothing to project from, and a template nothing declares — the second is the one that
# looks harmless, and is a file the generator will never read.
F="$(fixture 1)"
rm -f "$F/share/providers/codex.tmpl"
expect_exit 10 gate "$F"
expect_grep "provider 'codex' is in the declarations and share/providers/codex.tmpl does not exist"
expect_grep '1 finding\(s\); the rule is project.providers-are-data'

F="$(fixture 2)"
printf 'bootstrap for aider\n' > "$F/share/providers/aider.tmpl"
expect_exit 10 gate "$F"
expect_grep 'share/providers/aider.tmpl has no entry in share/providers.yaml'

# and the declaration and the generated document are two readings of one value: a provider
# added to the declaration and not regenerated is a document that has gone stale, and the
# gate says regenerate rather than reporting a missing provider
F="$(fixture 3)"
printf '  aider:\n    title: Aider Shell\n' >> "$F/share/providers.yaml"
expect_exit 10 gate "$F"
expect_grep "share/providers.yaml declares 'aider' and docs/generated/providers.json does not carry it; regenerate"

# restored: declaring it in all three places is accepted
F="$(fixture 4)"
printf '  aider:\n    title: Aider Shell\n' >> "$F/share/providers.yaml"
printf 'bootstrap for aider\n' > "$F/share/providers/aider.tmpl"
jq '.providers += [{"id":"aider","title":"Aider Shell","bootstraps":[{"target":"AIDER.md"}]}]' \
  "$F/docs/generated/providers.json" > "$F/docs/generated/providers.json.new"
mv "$F/docs/generated/providers.json.new" "$F/docs/generated/providers.json"
printf 'AIDER.md merge=derived\n' >> "$F/.gitattributes"
expect_exit 0 gate "$F"

# ---------------------------------------------------------------- 2. the merge driver
# A projected bootstrap that is not merge=derived is resolved by git's ordinary merge, and
# an ordinary merge of two generations of a generated file leaves a stamp that describes
# neither side. This is the finding with the longest fuse: it costs nothing until a merge.
F="$(fixture 5)"
printf 'CLAUDE.md merge=derived\n.codex/config.toml merge=derived\n' > "$F/.gitattributes"
expect_exit 10 gate "$F"
expect_grep '.gemini/settings.json is a projected bootstrap and .gitattributes does not mark it merge=derived'

# and a policy that projects through a provider nothing declares writes a bootstrap from a
# template that does not exist
F="$(fixture 6)"
printf '  - provider: aider\n' >> "$F/.ai/repo/policy.yaml"
expect_exit 10 gate "$F"
expect_grep "policy.yaml projects through 'aider', which no template and no declaration names"

# ---------------------------------------------------------------- 3. no list written by hand
# The heuristic, from both sides. A line that names three declared providers is a list the
# generated document already is, and the day a fourth provider lands that line is wrong with
# nothing to notice.
F="$(fixture 7)"
mkdir -p "$F/docs"
printf '# Clients\n\nMajordomus supports Claude, Codex and Gemini.\n' > "$F/docs/CLIENTS.md"
expect_exit 10 gate "$F"
expect_grep 'docs/CLIENTS.md enumerates 3 providers by hand and does not point at docs/generated/providers.md'
expect_grep 'Majordomus supports Claude, Codex and Gemini'

# the same line in a file that points at the projection is a reading aid beside the list,
# not a second list: the pointer is what makes it maintainable
F="$(fixture 8)"
mkdir -p "$F/docs"
printf '# Clients\n\nMajordomus supports Claude, Codex and Gemini.\n\nThe current set is docs/generated/providers.md.\n' \
  > "$F/docs/CLIENTS.md"
expect_exit 0 gate "$F"
expect_grep 'no document enumerates providers by hand without pointing at docs/generated/providers.md'

# and two names is a sentence. A gate that called this a list would make it impossible to
# write about two providers at once, which is most of what documentation about providers is.
F="$(fixture 9)"
mkdir -p "$F/docs"
printf '# Clients\n\nClaude and Codex read the same layer.\n' > "$F/docs/CLIENTS.md"
expect_exit 0 gate "$F"

# a provider named by its id rather than its title counts the same — the rule is about the
# list, not about the spelling
F="$(fixture 10)"
mkdir -p "$F/docs"
printf '# Clients\n\nThe ids are claude, codex and gemini.\n' > "$F/docs/CLIENTS.md"
expect_exit 10 gate "$F"
expect_grep 'docs/CLIENTS.md enumerates 3 providers by hand'

# ---------------------------------------------------------------- 4. the projection is current
# With an executable at hand the gate asks it whether the generated document matches the
# declaration. A stub stands in for the crate here: what is under test is that the gate
# *asks*, and reports the answer, rather than assuming.
F="$(fixture 11)"
STUB="$T/bin-ok"; mkdir -p "$(dirname "$STUB")"
printf '#!/usr/bin/env bash\nexit 0\n' > "$STUB"; chmod +x "$STUB"
( cd "$F" && git add -A >/dev/null 2>&1 ) || true
expect_exit 0 env MJ_ROOT="$F" MAJORDOMUS_BIN="$STUB" "$GATE"
expect_grep 'docs/generated/providers.\* are current \(generate --check providers\)'

STUB="$T/bin-stale"
printf '#!/usr/bin/env bash\nexit 1\n' > "$STUB"; chmod +x "$STUB"
expect_exit 10 env MJ_ROOT="$F" MAJORDOMUS_BIN="$STUB" "$GATE"
expect_grep 'docs/generated/providers.\* are stale: run majordomus generate providers'

# and without one the gate says who decides instead of passing silently — a check that
# cannot reach its subject must say so rather than report a verdict it does not have
expect_exit 0 gate "$F"
expect_grep 'no executable at hand; derive-check decides whether docs/generated/providers.\* are current'

# ---------------------------------------------------------------- an unusable tree
# The generated document is the gate's whole reading of the declaration. Without it, or with
# a document of another contract, there is nothing to check — 12, not a quiet 0.
F="$(fixture 12)"
rm -f "$F/docs/generated/providers.json"
expect_exit 12 gate "$F"
expect_grep 'docs/generated/providers.json is missing; run majordomus generate providers'

F="$(fixture 13)"
printf '{"schema":"majordomus/other/v1","providers":[]}\n' > "$F/docs/generated/providers.json"
expect_exit 12 gate "$F"
expect_grep 'is not a majordomus/providers/v1 document'

echo "    a provider is declared once: every projection held to it, and no list written by hand"
