# majordomus-covers: none
# The gate of project.github-projection-gated: does the remote still agree with the
# canonical project model?
#
# `scripts/github-sync --check` has exited 11 on drift since the adapter was written, and
# nothing ever called it. core-check ran --plan and --render, which prove a projection can
# be produced, not that the remote received one. So the projection decayed to a tenth of the
# model over five days with every build green.
#
# A gate that cannot be shown to fail is decoration, so each half is driven over a fixture
# repository with a fixture remote, offline: a canonical record that moved and was never
# projected must fail outright, a record added to the model and never projected must break
# the ratchet, and a backlog sitting exactly at its baseline must pass.
. "$ROOT/test/lib.sh"

GATE="$ROOT/scripts/ci/github-check"
SYNC="$ROOT/scripts/github-sync"

# ---------------------------------------------------------------- wired into the CI model
# The gate is selected by the path classes that can move either side of the projection:
# the canonical model under .ai/, and the adapter under scripts/.
expect_grep 'id: github-projection' "$ROOT/.ai/repo/ci/gates.yaml"
expect_grep 'scripts/ci/github-check' "$ROOT/.ai/repo/ci/gates.yaml"
python3 - "$ROOT/.ai/repo/ci/gates.yaml" <<'PY' || exit 1
import sys, yaml
m = yaml.safe_load(open(sys.argv[1]))
for want in ("layer", "shell"):
    c = next(c for c in m["classes"] if c["id"] == want)
    if "github-projection" not in c["gates"]:
        print("    class %s does not select github-projection" % want); sys.exit(1)
PY

# ---------------------------------------------------------------- a fixture repository
"$MJ" init >/dev/null
git add .gitignore >/dev/null 2>&1; git commit -qm "ignore local ai state" >/dev/null 2>&1 || true
pj_init
pj_milestone M000
pj_issue I0001 M000
pj_issue I0002 M000

FX_I="$T/issues.tsv"; FX_M="$T/ms.tsv"; BASE="$T/baseline.txt"
printf '1\tM000 — Milestone M000\topen\n' > "$FX_M"
row() {
  b="$(cat | base64 | tr -d '\n')"
  printf '%s\t%s\topen\t%s\tM000 — Milestone M000\t%s\n' "$1" "$2" "$b" "$3"
}
# The gate is the tool's; the model is this fixture repository, which is what MJ_ROOT says.
gate() {
  MJ_ROOT="$PWD" MJ_GH_FIXTURE_ISSUES="$FX_I" MJ_GH_FIXTURE_MILESTONES="$FX_M" \
    MJ_GH_BASELINE="$BASE" "$GATE"
}

# both issues projected exactly as the adapter would post them: nothing to report
{ "$SYNC" --render I0001 | row 7 'I0001 — Issue I0001' I0001
  "$SYNC" --render I0002 | row 8 'I0002 — Issue I0002' I0002; } > "$FX_I"
printf 'missing 0\nadopt 0\n' > "$BASE"
expect_exit 0 gate
expect_grep 'no record is behind'
expect_grep 'github-check: 0 finding'

# --- a canonical record moved and nobody projected it: refused outright, no tolerance
sed 's/^objective: .*/objective: "Moved after it was projected."/' \
  .ai/repo/project/issues/I0002.yaml > "$T/i.$$" && mv "$T/i.$$" .ai/repo/project/issues/I0002.yaml
expect_exit 10 gate
expect_grep 'FAIL github-check +1 record\(s\) behind'
expect_grep 'scripts/github-sync --check'

# --- put it back, and add a record the projection has never seen: the ratchet catches it
{ "$SYNC" --render I0001 | row 7 'I0001 — Issue I0001' I0001
  "$SYNC" --render I0002 | row 8 'I0002 — Issue I0002' I0002; } > "$FX_I"
expect_exit 0 gate
pj_issue I0003 M000
expect_exit 10 gate
expect_grep 'FAIL github-check +1 record\(s\) missing, over the baseline of 0'
expect_grep 'scripts/github-sync --apply'

# --- the baseline may be lowered, and lowering it is what a backfill leaves behind
printf 'missing 1\nadopt 0\n' > "$BASE"
expect_exit 0 gate
expect_grep '1 record\(s\) missing, at the baseline'
printf 'missing 5\nadopt 0\n' > "$BASE"
expect_exit 0 gate
expect_grep 'under the baseline of 5'

# --- a person's text is at stake: refused, and never ratcheted
{ "$SYNC" --render I0001 | row 7 'I0001 — Issue I0001' I0001
  "$SYNC" --render I0002 | sed 's/^| status |.*/| status | somebody typed this |/' \
    | row 8 'I0002 — Issue I0002' I0002
  "$SYNC" --render I0003 | row 9 'I0003 — Issue I0003' I0003; } > "$FX_I"
expect_exit 10 gate
expect_grep 'FAIL github-check +1 record\(s\) edited'

# --- a remote issue claiming a record this repository does not have is refused
{ "$SYNC" --render I0001 | row 7 'I0001 — Issue I0001' I0001
  "$SYNC" --render I0002 | row 8 'I0002 — Issue I0002' I0002
  "$SYNC" --render I0003 | row 9 'I0003 — Issue I0003' I0003
  "$SYNC" --render I0003 | sed 's/majordomus:record I0003/majordomus:record I9999/' \
    | row 10 'I9999 — Struck from the model' I9999; } > "$FX_I"
expect_exit 10 gate
expect_grep 'FAIL github-check +1 record\(s\) unmanaged'

# --- a gate with no baseline refuses to guess one
rm -f "$BASE"
expect_exit 12 gate
expect_grep 'no baseline'

# --- and this repository's own tree passes its own gate, against its own baseline
expect_grep 'missing' "$ROOT/.ai/repo/ci/github-drift-baseline.txt"
