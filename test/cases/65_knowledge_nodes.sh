# A node's identity survives a rebuild, and its kind is never guessed from prose.
#
# Both are refusals rather than features, and both fail quietly if they fail at all:
#
#   Identity from a content hash makes every edit delete a node and create a stranger, and
#   every reference to it points at nothing without anything saying so.
#
#   A kind inferred from words in a document is a confident answer that nobody can check.
#   The rule here is that a kind comes from the source class or from a field the file
#   declares, and where no rule applies the answer is `unknown` — emitted, not dropped.
. "$ROOT/test/lib.sh"
# The unknown-kind case mutates the shipped source list, so the case runs its own copy of
# the tool rather than the repository's.
mkdir -p "$T/tool"
fixture_repo "$T/tool" docs
MJ="$T/tool/bin/majordomus"
SRC="$T/tool/share/knowledge-sources.yaml"

"$MJ" init >/dev/null
mkdir -p docs
cat > docs/GUIDE.md <<'EOF'
# The operating guide

This document is about the roadmap, the milestone process and several policies. None of
those words makes it any of those things.
EOF
printf 'no heading here, only prose\n' > docs/UNTITLED.md
# a claims matrix: one file, many objects, each keyed by an id the repository already uses
cat > docs/CLAIMS.yaml <<'EOF'
version: 1
claims:
  - id: policy-parse
    claim: The canonical policy is parsed and rejected if it contains an unknown key
    source: docs/SCHEMAS.md
    implementation: lib/common.sh
    test: test/cases/00_yaml_flatten.sh
    status: guaranteed
  - id: scoped-task
    claim: A task declares the paths it may touch
    source: docs/DESIGN.md
    implementation: lib/start.sh
    test: test/cases/04_start_check.sh
    status: guaranteed
EOF
git add -A >/dev/null && git commit -q -m install

# --- a rebuild produces the same bytes
"$MJ" knowledge nodes > a.txt
"$MJ" knowledge nodes > b.txt
cmp -s a.txt b.txt || { echo "    two runs disagreed"; diff a.txt b.txt | head; exit 1; }

# --- identity is the canonical id where there is one, and the path where there is not
expect_grep '^policy +shared +[0-9a-f]{12} +policy:\.majordomus/policy\.yaml' a.txt
expect_grep '^profile +shared +[0-9a-f]{12} +profile:debugging' a.txt
expect_grep '^document +shared +[0-9a-f]{12} +document:docs/GUIDE\.md' a.txt

# --- identity is NOT the content hash: editing a source keeps the id and moves the hash
before_id="$(awk '$NF ~ /GUIDE/ || $4 == "document:docs/GUIDE.md" { print $4 }' a.txt | head -1)"
before_hash="$(awk '$4 == "document:docs/GUIDE.md" { print $3 }' a.txt)"
printf '\nAn added paragraph.\n' >> docs/GUIDE.md
git add -A >/dev/null && git commit -q -m edit
"$MJ" knowledge nodes > c.txt
after_hash="$(awk '$4 == "document:docs/GUIDE.md" { print $3 }' c.txt)"
[ -n "$before_id" ] || { echo "    the guide produced no node"; exit 1; }
grep -q 'document:docs/GUIDE\.md' c.txt || { echo "    editing a document changed its node id"; exit 1; }
[ "$before_hash" != "$after_hash" ] || { echo "    editing a document did not change its hash"; exit 1; }

# --- and editing one source moves no other node's hash
other_before="$(awk '$4 == "policy:.majordomus/policy.yaml" { print $3 }' a.txt)"
other_after="$(awk '$4 == "policy:.majordomus/policy.yaml" { print $3 }' c.txt)"
[ "$other_before" = "$other_after" ] || { echo "    editing one source changed another's hash"; exit 1; }

# --- a rename is a delete and an add of that document alone; nothing else moves
git mv docs/GUIDE.md docs/HANDBOOK.md && git commit -q -m rename
"$MJ" knowledge nodes > d.txt
expect_no_grep 'document:docs/GUIDE\.md' d.txt
expect_grep '^document +shared +[0-9a-f]{12} +document:docs/HANDBOOK\.md' d.txt
# every other id is untouched by the rename
awk '$1 != "document" || $4 !~ /GUIDE|HANDBOOK/ { print $4 }' c.txt | LC_ALL=C sort > ids-c.txt
awk '$1 != "document" || $4 !~ /GUIDE|HANDBOOK/ { print $4 }' d.txt | LC_ALL=C sort > ids-d.txt
cmp -s ids-c.txt ids-d.txt || { echo "    a rename disturbed unrelated node ids"; diff ids-c.txt ids-d.txt | head; exit 1; }

# --- kind comes from the source class, never from words in the body. The guide talks about
#     roadmaps, milestones and policies and is a document, like every other document.
kind="$(awk '$4 == "document:docs/HANDBOOK.md" { print $1 }' d.txt)"
[ "$kind" = document ] || { echo "    a document's kind was inferred from its prose: got '$kind'"; exit 1; }

# --- a title is taken from the first heading, and is left empty when there is none.
#     A basename standing in for a missing title is a guess that reads like a fact.
expect_grep 'document:docs/HANDBOOK\.md +The operating guide$' d.txt
line="$(grep 'document:docs/UNTITLED\.md' d.txt)"
[ -n "$line" ] || { echo "    an untitled document produced no node"; exit 1; }
case "$line" in
  *UNTITLED.md*[!\ ]*) echo "    an untitled document was given a title: $line"; exit 1 ;;
esac

# --- one file with many objects: every claim in the matrix is its own node, keyed by the id
#     the rest of the repository already refers to
expect_grep '^claim +shared +[0-9a-f]{12} +claim:policy-parse' d.txt

# --- a question keeps its identity when it is answered. Resolution rewrites the line it
#     lives on, so an identity taken from the whole line changes the moment somebody
#     answers, which is the one thing an identity must not do. The identity is the question
#     text, which is also what the ledger records and what a session envelope references.
"$MJ" start "ask something" --scope docs >/dev/null
"$MJ" question add "Which way round should the gate go?" >/dev/null
"$MJ" knowledge nodes --kind question > q1.txt
qid="$(awk '{ print $4 }' q1.txt | grep '^question:' | head -1)"
[ -n "$qid" ] || { echo "    an open question produced no node"; cat q1.txt; exit 1; }
"$MJ" question resolve "Which way round" --answer "Fail closed." >/dev/null
"$MJ" knowledge nodes --kind question > q2.txt
grep -qF "$qid" q2.txt || {
  echo "    answering a question changed its node identity"
  echo "    was: $qid"; echo "    now: $(awk '{print $4}' q2.txt | grep '^question:' | head -1)"; exit 1; }
# and the answer is not part of the identity
expect_no_grep 'Fail closed' q2.txt

# --- a source class whose kind this extractor has no rule for yields `unknown`, and the
#     node is still emitted. Dropping it would hide the source; guessing would invent a
#     type nobody declared.
cat >> "$SRC" <<'EOF'

  - id: oddity
    kind: contraption
    scope: shared
    discovery: vcs
    pathspec: ':(glob)docs/UNTITLED.md'
    required: false
EOF
grep -q 'contraption' "$SRC" || { echo "    the source-list mutation did not take"; exit 1; }
expect_exit 0 "$MJ" knowledge nodes
expect_grep 'unknown +shared +[0-9a-f]{12} +unknown:docs/UNTITLED\.md'
expect_grep 'WARN +knowledge +oddity — source class declares kind .contraption.'

# --- two objects claiming one identity is a failure, not a last-one-wins merge
cat >> "$SRC" <<'EOF'

  - id: twin
    kind: policy
    scope: shared
    discovery: vcs
    pathspec: ':(glob).majordomus/policy.yaml'
    required: false
EOF
expect_exit 10 "$MJ" knowledge nodes
expect_grep 'FAIL +knowledge +policy:\.majordomus/policy\.yaml — claimed by'

# --- read-only: deriving nodes writes nothing
git -C . checkout -- . 2>/dev/null || true
rm -f a.txt b.txt c.txt d.txt ids-c.txt ids-d.txt q1.txt q2.txt
before_status="$(git status --porcelain)"
"$MJ" knowledge nodes >/dev/null 2>&1 || true
"$MJ" knowledge nodes --scope shared >/dev/null 2>&1 || true
"$MJ" --json knowledge nodes >/dev/null 2>&1 || true
[ "$(git status --porcelain)" = "$before_status" ] \
  || { echo "    deriving nodes changed the working tree"; git status --porcelain; exit 1; }

# --- JSON carries what a caller needs to explain a node without re-deriving it
expect_exit 0 "$MJ" --json knowledge nodes --scope shared --kind claim
expect_grep '"id":"claim:policy-parse"'
expect_grep '"kind":"claim"'
expect_grep '"source":"docs/CLAIMS.yaml"'
expect_grep '"hash":"[0-9a-f]{64}"'
