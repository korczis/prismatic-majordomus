# majordomus-covers: knowledge
# majordomus-negative: knowledge
# A curated note the schema refuses is not knowledge, whichever reader is asked.
#
# The schema of the `knowledge` kind requires `class` and closes its values. The executable's
# index held that — it refused such a note and made no object of it — while the shell's
# `knowledge nodes` read the same note as prose and emitted a node. Two readers of one layer
# disagreed about what the layer holds, and the one people run by hand was the lenient one
# (ADR 0010). The reader now asks the index which files it refused, drops them, and reports
# the index's own verdict: one judge, not two opinions.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_BIN="$RB"

"$MJ" init >/dev/null
C=.ai/repo/knowledge/curated
mkdir -p "$C"
note() {
  cat <<EOF
---
schema: knowledge/v1
id: $1
kind: knowledge
$2
title: A note called $1
description: A fixture note.
status: verified
epistemics: decided
date: 2026-09-19
provenance:
  origin: authored
---

# Note $1
EOF
}
note valid 'class: convention' > "$C/valid.md"
note classless '' > "$C/classless.md"
note rumour 'class: rumour' > "$C/rumour.md"
git add -A >/dev/null && git commit -q -m notes

# --- the refused notes yield no node, and the refusal names the file and the constraint
expect_exit 10 "$MJ" knowledge nodes --kind knowledge
expect_grep 'knowledge:\.ai/repo/knowledge/curated/valid\.md'
expect_no_grep '^knowledge .*knowledge:\.ai/repo/knowledge/curated/classless\.md'
expect_no_grep '^knowledge .*knowledge:\.ai/repo/knowledge/curated/rumour\.md'
expect_grep 'curated/classless\.md — the index refused it \(schema_violation\).*"class" is a required property'
expect_grep 'curated/rumour\.md — the index refused it \(schema_violation\).*"rumour"'

# --- the same verdict reaches the machine-readable form
"$MJ" knowledge nodes --json > nodes.json 2>/dev/null || true
[ "$(jq -r '[.findings[] | select(.code == "refused_source")] | length' nodes.json)" = 2 ] \
  || { echo "    --json does not carry both refusals"; jq '.findings' nodes.json; exit 1; }

# --- both readers now agree on the knowledge node set over the same tree
shell_set="$(jq -r '.nodes[] | select(.kind == "knowledge") | .source' nodes.json | LC_ALL=C sort)"
index_set="$(MAJORDOMUS_SHARE="$ROOT/share" "$RB" run objects.list --input '{"kind":"knowledge"}' --repo . --format json 2>/dev/null \
  | jq -r '.output.objects[] | .path' | LC_ALL=C sort)"
[ -n "$index_set" ] || { echo "    the index returned no knowledge object; the comparison proves nothing"; exit 1; }
[ "$shell_set" = "$index_set" ] || {
  echo "    the shell reader and the index disagree"
  printf '      shell: %s\n      index: %s\n' "$shell_set" "$index_set"
  exit 1
}

# --- a conforming layer is quiet: removing the two refused notes leaves no finding
git rm -q "$C/classless.md" "$C/rumour.md" && git commit -q -m conforming
expect_exit 0 "$MJ" knowledge nodes --kind knowledge
expect_no_grep 'the index refused it'

# --- with no executable to ask, the reader says the schema went unchecked; it never passes
MAJORDOMUS_BIN="$PWD/no-such-executable" "$MJ" knowledge nodes --json > unchecked.json 2>/dev/null || true
[ "$(jq -r '[.findings[] | select(.code == "schema_unchecked")] | length' unchecked.json)" = 1 ] \
  || { echo "    an unasked index reads as a checked one"; jq '.findings' unchecked.json; exit 1; }
