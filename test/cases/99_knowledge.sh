# majordomus-covers: doctor
# The repository knowledge system through the built executable, in a disposable repository:
# the check refuses everything before adoption, one command records the present state as the
# baseline and the check passes, a curated claim whose evidence moves is refused by name until
# a person accepts it, a second mirror of the registry is a canonicality violation with exit
# 10, and the two new kinds validate in the layer like every other. The crate's own suite
# (apps/majordomus-cli/tests/knowledge.rs) covers the model in depth; what is here is the
# part that crosses the two programs: the shell tool's doctor over the files the Rust
# executable writes, and the exit-code contract a hook and a gate read.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
command -v jq >/dev/null 2>&1 || { echo "    skip: jq not installed"; exit 0; }

R="$T/repo"
fixture_repo "$R" >/dev/null
# the fixture carries this repository's own baseline and generated manifest: the case adopts
# from nothing, and the manifest names artifacts the fixture does not copy
rm -f "$R/.ai/repo/knowledge/baseline.yaml" "$R/.ai/repo/knowledge/exceptions.yaml" "$R/docs/generated/artifacts.json"
# a curated record verified against a document, and a document that links to nowhere
cat > "$R/.ai/repo/knowledge/curated/case-note.md" <<'EOF'
---
schema: knowledge/v1
id: case-note
kind: knowledge
class: fact
title: The CLI document opens with its title
description: A note verified against the document it names.
status: verified
epistemics: observed
date: 2026-01-01
provenance:
  origin: authored
  derived_from:
    - file:docs/CLI.md
---

# The note

Verified against docs/CLI.md.
EOF
printf '# A guide\n\nSee [the runbook](runbooks/none.md), which nobody wrote.\n' > "$R/docs/GUIDE.md"
git -C "$R" init -q .
git -C "$R" config user.email t@example.com
git -C "$R" config user.name t
git -C "$R" add -A >/dev/null
git -C "$R" commit -qm fixture >/dev/null

mj() { ( cd "$R" && MAJORDOMUS_SHARE="$R/share" "$RB" "$@" ); }

# ---------------------------------------------------------------- before adoption
expect_exit 10 mj knowledge check
expect_grep 'no baseline recorded'
expect_grep 'knowledge:case-note'
expect_exit 0 mj knowledge stale
expect_grep 'knowledge:case-note'
expect_grep 'document:docs/GUIDE.md'
expect_exit 0 mj knowledge status
expect_grep '^baseline +not recorded'
expect_grep '^check +fail'

# ---------------------------------------------------------------- adoption
expect_exit 0 mj knowledge bootstrap
expect_grep 'baseline recorded at .ai/repo/knowledge/baseline.yaml'
[ -f "$R/.ai/repo/knowledge/baseline.yaml" ] || { echo "    no baseline written"; exit 1; }
grep -q '^schema: majordomus/knowledge-baseline/v1$' "$R/.ai/repo/knowledge/baseline.yaml" || { echo "    the baseline carries no schema line"; exit 1; }
grep -q 'document:docs/GUIDE.md' "$R/.ai/repo/knowledge/baseline.yaml" || { echo "    the present debt is not tolerated by name"; exit 1; }
! grep -q "$R" "$R/.ai/repo/knowledge/baseline.yaml" || { echo "    a machine path in a tracked file"; exit 1; }
expect_exit 0 mj knowledge check
expect_grep 'knowledge check: pass'
expect_exit 10 mj knowledge bootstrap
expect_grep 'a baseline is recorded'
# the layer reads the file the executable wrote as a kind of its own
( cd "$R" && expect_exit 0 "$MJ" doctor ) || exit 1
expect_grep 'doctor: 0 failure'
( cd "$R" && expect_exit 0 "$MJ" knowledge sources ) || exit 1
expect_grep '^knowledge_baseline +shared +knowledge-baseline'

# ---------------------------------------------------------------- the evidence moves
printf '\nA new paragraph.\n' >> "$R/docs/CLI.md"
git -C "$R" commit -qam "edit the document" >/dev/null
expect_exit 10 mj knowledge check
expect_grep 'knowledge:case-note'
expect_grep 'stale'
expect_exit 0 mj knowledge explain knowledge:case-note
expect_grep 'changed since this was verified'
expect_grep 'reconcile --accept'
expect_exit 0 mj knowledge reconcile --accept
expect_grep 'baseline written'
expect_exit 0 mj knowledge check
expect_grep 'knowledge check: pass'

# ---------------------------------------------------------------- canonicality
expect_exit 0 mj canonicality
expect_grep 'canonicality verdict: pass'
expect_exit 0 mj explain capability objects.get
expect_grep 'canonical source: apps/majordomus-cli/src/capability/builtin/objects.rs'
expect_grep '✓ mcp'
printf '# Mirror\n\n- `objects.get`\n- `objects.list`\n- `objects.search`\n- `capabilities.list`\n- `capabilities.describe`\n- `health.report`\n' > "$R/docs/MIRROR.md"
git -C "$R" add -A >/dev/null; git -C "$R" commit -qm "a mirror" >/dev/null
expect_exit 10 mj canonicality check
expect_grep 'suspected_mirror'
expect_grep 'docs/MIRROR.md'
expect_exit 10 mj knowledge check
expect_grep 'canonicality:suspected_mirror:docs/MIRROR.md'
expect_exit 10 mj change inspect --base HEAD~1
expect_grep 'change inspect: fail'
expect_grep 'docs/MIRROR.md'
# a typed exception with a date covers it; the audit says by whom
cat > "$R/.ai/repo/knowledge/exceptions.yaml" <<'EOF'
schema: majordomus/knowledge-exceptions/v1
exceptions:
  - id: suspected_mirror:docs/MIRROR.md
    reason: the table is being derived from the registry
    owner: the maintainers
    expires: 2999-01-01
    validation: docs/MIRROR.md names no capability by hand
EOF
expect_exit 0 mj canonicality check
expect_grep 'excepted by suspected_mirror:docs/MIRROR.md'
expect_exit 0 mj knowledge check
( cd "$R" && expect_exit 0 "$MJ" doctor ) || exit 1
expect_grep 'doctor: 0 failure'

# ---------------------------------------------------------------- the model as data
mj knowledge scan --public > "$T/public.json"
jq -e '.schema == "majordomus/knowledge/v1" and (.nodes | length) > 10 and all(.evidence[]; .visibility == "public")' "$T/public.json" >/dev/null \
  || { echo "    the public projection is not the document it claims"; exit 1; }
mj knowledge scan > "$T/a.json"; mj knowledge scan > "$T/b.json"
cmp -s "$T/a.json" "$T/b.json" || { echo "    two scans of one tree differ"; exit 1; }
