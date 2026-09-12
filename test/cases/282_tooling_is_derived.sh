# majordomus-covers: update doctor
# majordomus-negative: doctor
# claim: tooling-derived
# AI client instructions are derived, never maintained by hand (ADR 0057, rule
# project.tooling-is-derived). The definition of done reaches every rendered bootstrap as a
# fragment of share/completion.yaml; the shell renderer and the executable produce the same
# bytes; a change to the policy's stages makes the committed render stale until it is
# regenerated; and a hand edit is refused by doctor.
. "$ROOT/test/lib.sh"
BIN="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_BIN="$BIN" MAJORDOMUS_SHARE="$ROOT/share"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj282.XXXXXX")"; trap 'rm -rf "$S"' EXIT
git config user.email a@b.c; git config user.name a
git config core.hooksPath .githooks; mkdir -p .githooks
printf '#!/bin/sh\nPATH="%s:$PATH"; exec majordomus doctor\n' "$(dirname "$MJ")" > .githooks/pre-commit; chmod +x .githooks/pre-commit
printf '#!/bin/sh\nPATH="%s:$PATH"; exec majordomus finish --check\n' "$(dirname "$MJ")" > .githooks/pre-push; chmod +x .githooks/pre-push

"$MJ" init >/dev/null; "$MJ" update >/dev/null
git add -A >/dev/null && git commit -qm base

# ---------------------------------------------------------------- the fragment is in the render
for f in AGENTS.md CLAUDE.md .bb/AGENTS.md; do
  [ -f "$f" ] || continue
  grep -q '^- Live verification: deployment-verified$' "$f" \
    || { echo "    $f carries no definition of done from share/completion.yaml"; exit 1; }
  grep -q 'Done is not a claim' "$f" || { echo "    $f does not say that done is not a claim"; exit 1; }
  grep -qE '^- \*\*' "$f" && { echo "    $f carries rule bullets, which doctor refuses"; exit 1; }
done
# every stage with a question is one line, in the policy's order, and nothing is written by hand
awk '/^stages:/{s=1;next} /^questions:/{s=0} s && /^    title: /{sub(/^    title: /,""); print}' "$ROOT/share/completion.yaml" > "$S/titles"
grep '^- [A-Z].*: ' AGENTS.md | sed 's/^- //; s/:.*//' > "$S/rendered"
grep -vxF -f "$S/rendered" "$S/titles" | grep -vx 'Context and governance' > "$S/missing" || true
[ -s "$S/missing" ] && { echo "    stages missing from the render: $(cat "$S/missing" | paste -sd, -)"; exit 1; }

# ---------------------------------------------------------------- both renderers agree, byte for byte
expect_exit 0 "$BIN" generate providers --check --repo .
expect_grep 'in sync'
"$BIN" run gates.policy --input '{}' --quiet --format json --repo . 2>/dev/null | jq -r '.output.fragment' | sed '$d' > "$S/fragment.rs"
grep '^- [A-Z].*: ' AGENTS.md > "$S/fragment.sh"
cmp -s "$S/fragment.rs" "$S/fragment.sh" || { echo "    the two renderers disagree:"; diff "$S/fragment.rs" "$S/fragment.sh"; exit 1; }

# ---------------------------------------------------------------- a moved source makes the render stale
cp -R "$ROOT/share" "$S/share"
sed -i.bak 's/^    title: Live verification$/    title: Verification in production/' "$S/share/completion.yaml" && rm "$S/share/completion.yaml.bak"
expect_exit 10 env MAJORDOMUS_SHARE="$S/share" "$BIN" generate providers --check --repo .
expect_grep 'AGENTS.md'
# regenerating from the moved source is the remedy, and the only one
expect_exit 0 env MAJORDOMUS_SHARE="$S/share" "$MJ" update
grep -q '^- Verification in production: deployment-verified$' AGENTS.md \
  || { echo "    the regenerated bootstrap does not carry the moved stage"; exit 1; }
expect_exit 0 env MAJORDOMUS_SHARE="$S/share" "$BIN" generate providers --check --repo .
expect_exit 0 env MAJORDOMUS_SHARE="$S/share" "$MJ" doctor
# and back
expect_exit 0 "$MJ" update
expect_exit 0 "$BIN" generate providers --check --repo .

# ---------------------------------------------------------------- a hand edit is refused
printf '\n- Always deploy on Fridays.\n' >> AGENTS.md
expect_exit 10 "$MJ" doctor
expect_grep 'AGENTS.md.*hand'
expect_exit 0 "$MJ" update --force
expect_exit 0 "$MJ" doctor

# ---------------------------------------------------------------- a policy problem is reported, not hidden
sed -i.bak 's/^    source: obligation:pages$/    source: obligation:no-such-token/' "$S/share/completion.yaml" && rm "$S/share/completion.yaml.bak"
MAJORDOMUS_SHARE="$S/share" "$BIN" run gates.policy --input '{}' --quiet --format json --repo . 2>/dev/null | jq -r '.output.problems[]' | grep -q 'no-such-token' \
  || { echo "    a question naming a token the vocabulary lacks is not a reported problem"; exit 1; }
echo "    ok"
