# majordomus-covers: none
# `knowledge nodes` refuses an identity two objects claim — case 73 proves the refusal — and
# for as long as that refusal existed nothing asked for it. On 2026-09-13 it exited 10 over
# this repository with a directory contract claimed twice in every section, and every gate
# was green: a reporter that no plan runs is a declaration, not an enforcement.
#
# So the property here is not that the command detects a collision. It is that the CI model
# runs it on every plan, in the structure job, with a command line that lets its exit through,
# and that the command line the model names turns a planted collision into a failing gate.
#
# The planted collision is two different files claiming one identity: two decision records
# carrying the same ADR number. Neither a tie-break between classes nor a dedupe of one file
# discovered twice can settle that, because the two claims disagree about what the object is.
# Two classes of different kinds on one file are not a collision at all (the identity is
# <kind>:<path>), and one file reached by two classes of one kind is one fact found twice, so
# a plant of either shape would test a refusal the graph is right not to make.
. "$ROOT/test/lib.sh"
G="$ROOT/.ai/repo/ci/gates.yaml"

# ---------------------------------------------------------------- wired into the CI model
block="$(awk '$0 == "  - id: knowledge-nodes" {f=1; next} f && /^  - id: / {f=0} f' "$G")"
[ -n "$block" ] || { echo "    gates.yaml declares no knowledge-nodes gate"; exit 1; }
printf '%s\n' "$block" | grep -qE '^    job: structure$' \
  || { echo "    the knowledge-nodes gate is not in the structure job"; exit 1; }
# case 94 proves every always-gate is in every plan, the empty one included
printf '%s\n' "$block" | grep -qE '^    always: true$' \
  || { echo "    the knowledge-nodes gate does not run on every plan (always: true)"; exit 1; }
runs="$(printf '%s\n' "$block" | sed -n 's/^    runs: //p')"
case "$runs" in
  "bin/majordomus knowledge nodes"*) ;;
  *) echo "    the knowledge-nodes gate does not run bin/majordomus knowledge nodes: $runs"; exit 1 ;;
esac

# ---------------------------------------------------------------- the gate's own command line
# run-plan executes a gate as `bash -c "<runs>"` from the repository root; here the root is a
# disposable repository and bin/majordomus is the tool under test.
gate="$MJ${runs#bin/majordomus}"

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm init >/dev/null
# a repository with nothing claimed twice passes, so the gate is not red by construction
expect_exit 0 bash -c "$gate"

mkdir -p .ai/repo/adrs
for f in 0007-one-decision 0007-another-decision; do
  printf -- '---\nid: ADR-0007\nstatus: proposed\n---\n# %s\n\nTwo records, one decision number.\n' "$f" \
    > ".ai/repo/adrs/$f.md"
done
git add .ai/repo/adrs >/dev/null
expect_exit 10 bash -c "$gate"
expect_grep 'FAIL +knowledge +adr:ADR-0007 — claimed by \.ai/repo/adrs/0007-[a-z-]+\.md and by \.ai/repo/adrs/0007-[a-z-]+\.md'

echo "    ok: the knowledge graph's refusal is a gate on every plan, and a planted collision fails it"
