# majordomus-covers: none
# A directory contract is the kind it declares, and it is that kind once.
#
# `share/kinds.yaml` states it as a rule of the layer — `declared: [context]`, with the
# sentence "A README inside the rules tree that says `kind: context` is a context document,
# not a broken rule". Two things had to be true for that to hold and neither was:
#
#   1. the kind was forced to `document` — the opposite of the declaration — because
#      `context` was not in the extractor's assertable kinds, so every contract in the layer
#      was read as prose that happens to sit in a directory;
#   2. every class that discovered the file still produced a row, so one path arrived twice
#      under one identity and the extractor refused both: eleven
#      `document:<path> claimed by <path> and by <path>` failures in this repository, each
#      naming the same path on both sides because the claimants differed only in the class,
#      which the message never printed.
#
# What is asserted here is behaviour a reader can check: the node carries the declared kind,
# a second claimant adds nothing, and — the mutation that makes this a test rather than a
# restatement — removing the declaration puts the file back under its class's kind. The last
# one is what would fail if somebody restored `kind=document`: the first two would still
# pass, because a forced kind is also a consistent one.
. "$ROOT/test/lib.sh"

T_REPO="$T/contract"
mkdir -p "$T_REPO"
cd "$T_REPO" || { echo "    could not enter the fixture"; exit 1; }
git init -q .
printf 'x\n' > README.md
git add -A && git -c user.email=t@t -c user.name=t commit -qm init >/dev/null
"$MJ" init >/dev/null 2>&1 || { echo "    majordomus init failed in the fixture"; exit 1; }
git add -A && git -c user.email=t@t -c user.name=t commit -qm layer >/dev/null

C=".ai/repo/rules/README.md"
[ -f "$C" ] || { echo "    the skeleton shipped no $C to read"; exit 1; }
grep -q '^kind: context$' "$C" || { echo "    $C does not declare itself a context document"; exit 1; }

nodes() { "$MJ" knowledge nodes 2>&1; }
node_line() { nodes | grep -E "^[a-z-]+ +[a-z]+ +[0-9a-f]+ +[a-z-]+:$(printf '%s' "$1" | sed 's/[.[\*^$]/\\&/g')( |$)" ; }

# --- 1. the declaration decides the kind
out="$(nodes)"; rc=$?
[ "$rc" = 0 ] || { echo "    knowledge nodes exited $rc on a fresh layer:"; printf '%s\n' "$out" | grep -E '^FAIL' | head -3; exit 1; }
printf '%s\n' "$out" | grep -qE "^context .*context:$C" \
  || { echo "    $C is not a context node; the class it sits in decided its kind:"; printf '%s\n' "$out" | grep -F "$C" | head -2; exit 1; }
printf '%s\n' "$out" | grep -qE "document:$C|rule:$C" \
  && { echo "    $C is also claimed under another kind"; printf '%s\n' "$out" | grep -F "$C" | head -3; exit 1; }
echo "    a README that declares kind: context is a context node, not an instance of its directory's kind"

# --- 2. the mutation: take the declaration away and the class decides again
# This runs before the second claimant is planted, and the order is the point. With a class
# whose kind is literally `context` in the list, a file stripped of its declaration would
# still be a context node — correctly, decided by the class — and the mutation would prove
# nothing. `init` declares exactly that class (`.ai/**/README.md`), so it is taken out in the
# same commit: the only classes claiming this path are then the directory's own, so what the
# node becomes is what the class says, which is what "the declaration decides" has to mean.
S=".ai/repo/knowledge/sources.yaml"
pre="$(git rev-parse HEAD)"
awk '/^  - id: context$/ { skip = 1; next } skip && /^  - id: / { skip = 0 } !skip' "$S" > "$S.tmp" && mv "$S.tmp" "$S"
grep -q '^    kind: context$' "$S" && { echo "    the fixture still declares a context class; the mutation would prove nothing"; exit 1; }
sed '/^kind: context$/d' "$C" > "$C.tmp" && mv "$C.tmp" "$C"
git add -A && git -c user.email=t@t -c user.name=t commit -qm "the contract stops declaring itself" >/dev/null
# Without its declaration the file is a contract the manifest names that no longer says it is
# one, and the index refuses it (`missing_context_contract`). The knowledge reader asks the index
# which files it refused (ADR 0010, case 405), so that verdict is what this command must now
# give: exit 10, the file named as refused, and no node of any kind for it.
rc=0; gone="$(nodes)" || rc=$?
[ "$rc" = 10 ] || { echo "    a contract stripped of its declaration made knowledge nodes exit $rc, not 10"; exit 1; }
printf '%s\n' "$gone" | grep -qE "^FAIL .*$C — the index refused it \(missing_context_contract\)" \
  || { echo "    $C was not reported as refused by the index:"; printf '%s\n' "$gone" | grep -F "$C" | head -3; exit 1; }
printf '%s\n' "$gone" | grep -qE "^[a-z-]+ +[a-z]+ +[0-9a-f]+ +[a-z-]+:$C( |$)" \
  && { echo "    $C is still a node after its declaration was removed"; printf '%s\n' "$gone" | grep -F "$C" | head -2; exit 1; }
echo "    removing the declaration makes the contract a refused file, reported with exit 10 and never a node"
# From the commit before the mutation, not from the index: the mutation was committed, so
# `git checkout -- <path>` would restore the mutated file and the guard below would be the
# only thing that noticed. The commit is allowed to be empty-handed for the same reason a
# restore is allowed to be a no-op — under `bash -eu` a `git commit` with nothing staged
# exits 1 and takes the whole case with it, silently, which is how this block failed first.
git show "$pre":"$C" > "$C"
git show "$pre":"$S" > "$S"
git add -A && git -c user.email=t@t -c user.name=t commit -qm "the contract declares itself again" >/dev/null 2>&1 || true
grep -q '^kind: context$' "$C" || { echo "    the fixture could not be restored"; exit 1; }
# compared with the commit it was taken from rather than grepped for the class, so the case
# reads the same on a tree whose skeleton declares no context class at all
git show "$pre":"$S" | cmp -s - "$S" || { echo "    the fixture's source classes could not be restored"; exit 1; }

# --- 3. and it decides it once, however many classes discover the file
# The rules class already claims `.ai/repo/rules/**/*.md`. Adding a context class over
# `.ai/**/README.md` means two classes discover this one file, which is the shape that
# produced the eleven failures.
cat >> .ai/repo/knowledge/sources.yaml <<'YAML'

  - id: context_all
    kind: context
    discovery: vcs
    pathspec: ':(glob).ai/**/README.md'
    required: false
YAML
git add -A && git -c user.email=t@t -c user.name=t commit -qm "a second class claims the contracts" >/dev/null

two="$(nodes)"; rc=$?
[ "$rc" = 0 ] || { echo "    a second claimant made knowledge nodes exit $rc:"; printf '%s\n' "$two" | grep -E '^FAIL' | head -3; exit 1; }
printf '%s\n' "$two" | grep -q 'duplicate_id' \
  && { echo "    two classes discovering one contract produced a duplicate identity:"; printf '%s\n' "$two" | grep 'duplicate_id' | head -2; exit 1; }
n="$(printf '%s\n' "$two" | grep -cE "context:$C( |$)")"
[ "$n" = 1 ] || { echo "    $C yielded $n nodes, not 1, with two classes claiming it"; exit 1; }
echo "    a second class discovering the same contract adds no second claim"

echo "    a contract is the kind it declares, once, whatever discovered it"
