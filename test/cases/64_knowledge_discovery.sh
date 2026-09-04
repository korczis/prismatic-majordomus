# Knowledge discovery is driven by the repository index, not by a filesystem walk.
#
# A walk returns build output, vendored trees and editor droppings, returns them in an
# order that differs between two machines, and can return a file nobody meant to publish.
# Every property this case asserts is a consequence of that one decision.
. "$ROOT/test/lib.sh"

"$MJ" init >/dev/null
mkdir -p docs
printf '# Docs\n' > docs/README.md
git add -A >/dev/null && git commit -q -m "install"

# --- an untracked file is not a source, however plausible it looks
printf '# Not committed\n' > docs/UNTRACKED.md
"$MJ" knowledge sources > sources.txt
grep -q 'docs/UNTRACKED.md' sources.txt && { echo "    an untracked file became a source"; exit 1; }
grep -q 'docs/README.md' sources.txt || { echo "    a tracked document was not discovered"; exit 1; }

# --- generated and vendored noise is excluded for free, because it is not tracked
mkdir -p node_modules/pkg site/public
printf '# vendored\n' > node_modules/pkg/README.md
printf '# built\n' > site/public/index.md
"$MJ" knowledge sources > sources.txt
expect_no_grep 'node_modules' sources.txt
expect_no_grep 'site/public' sources.txt

# --- discovery is deterministic: same repository, same answer, twice
"$MJ" knowledge sources > a.txt
"$MJ" knowledge sources > b.txt
cmp -s a.txt b.txt || { echo "    two runs disagreed"; diff a.txt b.txt | head; exit 1; }

# --- and it does not depend on filesystem order: touching a file changes nothing
touch docs/README.md
"$MJ" knowledge sources > c.txt
cmp -s a.txt c.txt || { echo "    modification time changed the discovery order"; exit 1; }

# --- every row carries the class, the scope, the kind, a content hash and the path
expect_grep '^policy +shared +policy +[0-9a-f]{12} +\.majordomus/policy\.yaml$' a.txt

# --- editing a source changes that source's hash and no other
before="$(awk '$5==".majordomus/policy.yaml"{print $4}' a.txt)"
other_before="$(awk '$5=="docs/README.md"{print $4}' a.txt)"
printf '\n# a comment the parser ignores\n' >> .majordomus/policy.yaml
"$MJ" knowledge sources > d.txt
after="$(awk '$5==".majordomus/policy.yaml"{print $4}' d.txt)"
other_after="$(awk '$5=="docs/README.md"{print $4}' d.txt)"
[ "$before" != "$after" ] || { echo "    editing a source did not change its hash"; exit 1; }
[ "$other_before" = "$other_after" ] || { echo "    editing one source changed another's hash"; exit 1; }
git checkout -- .majordomus/policy.yaml 2>/dev/null || true

# --- shared and operational are separable, and the separation is the publication boundary
"$MJ" start "work" --scope docs >/dev/null
printf 'progress\n' | "$MJ" checkpoint >/dev/null
"$MJ" knowledge sources --scope shared > shared.txt
"$MJ" knowledge sources --scope operational > op.txt
expect_no_grep '\.majordomus/state/' shared.txt
expect_no_grep ' operational ' shared.txt
grep -q '\.majordomus/state/checkpoints/' op.txt || { echo "    an operational record was not discovered"; exit 1; }
expect_no_grep ' shared ' op.txt

# --- a pathname with a space, a quote and a non-ASCII character survives discovery whole
mkdir -p docs
printf '# Odd\n' > "docs/a file with spaces and ěščř.md"
git add -A >/dev/null && git commit -q -m "odd name"
"$MJ" knowledge sources --scope shared > shared.txt
grep -qF 'docs/a file with spaces and ěščř.md' shared.txt \
  || { echo "    a pathname with spaces was lost or split"; exit 1; }
n="$(grep -cF 'a file with spaces' shared.txt)"
[ "$n" = 1 ] || { echo "    a pathname with spaces produced $n rows, expected 1"; exit 1; }

# --- the classes do not overlap: one file is discovered by exactly one class.
#     This is the publication boundary, not tidiness. Dropping the :(glob) prefix from the
#     root-document pathspec makes * cross a directory separator, at which point *.md also
#     matches .majordomus/state/handovers/*.md — and every handover in the checkout is
#     discovered as shared repository knowledge. The mutation was run; this case fails on it.
dupes="$(awk '{print $NF}' shared.txt | sort | uniq -d)"
[ -z "$dupes" ] || { echo "    a file was discovered by two classes: $dupes"; exit 1; }

# --- a required class that finds nothing is a finding, not a silence
rm -f .majordomus/profiles/*.yaml
expect_exit 0 "$MJ" knowledge sources
expect_grep 'WARN +knowledge +profile — required source class discovered nothing'

# --- read-only means read-only: nothing under discovery writes
git checkout -- .majordomus 2>/dev/null || true
rm -f a.txt b.txt c.txt d.txt shared.txt op.txt sources.txt docs/UNTRACKED.md
rm -rf node_modules site
before_status="$(git status --porcelain)"
"$MJ" knowledge sources >/dev/null
"$MJ" knowledge sources --scope shared >/dev/null
"$MJ" --json knowledge sources >/dev/null
[ "$(git status --porcelain)" = "$before_status" ] \
  || { echo "    a read-only knowledge command changed the working tree"; git status --porcelain; exit 1; }

# --- JSON output names every field a caller needs to explain a row
expect_exit 0 "$MJ" --json knowledge sources --scope shared
expect_grep '"class":"policy"'
expect_grep '"scope":"shared"'
expect_grep '"hash":"[0-9a-f]{64}"'
expect_grep '"empty_required":\['

# --- an unknown scope is a usage error, not a silently empty answer
expect_exit 2 "$MJ" knowledge sources --scope everything
expect_grep 'must be shared, operational or all'
