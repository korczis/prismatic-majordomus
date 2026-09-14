# majordomus-covers: none
# A distribution belongs to a checkout, and the gate dispatcher must not run a gate against
# another one's.
#
# `MAJORDOMUS_SHARE` names the distribution a command reads its schemas and allow-lists
# from. direnv exports the primary checkout's into every linked worktree, so `run-plan`
# dispatched from a worktree used to hand every gate a share belonging to a different tree.
# What the gates then reported was not this tree's drift but what two trees disagree about:
# on 2026-09-12 a session running the structure set from a worktree saw ten gates fail, and
# re-running them one at a time with `env -u MAJORDOMUS_SHARE` showed two of them passing —
# `adr-collision` had read an ADR collision that did not exist, and `claim-proof` had called
# a policy block invalid because the other checkout's schema predated it. A false red is the
# thing that teaches people to stop reading a gate, so this is worse than a missing check.
#
# Two gate scripts had already cleared the variable by hand for their own commands
# (`scripts/ci/design-check`, `scripts/ci/generation-converges`). That duplication is the
# argument for clearing it once in the dispatcher rather than in each gate that remembers.
#
# What this case proves is the three behaviours, because a guard whose failure nobody has
# watched proves nothing:
#
#   - a share naming another checkout is cleared, and says so
#   - a share naming this checkout's own share passes without a word
#   - MJ_KEEP_SHARE=1 keeps it, for deliberately running the gates against a distribution
#
# and, separately, that the gate the dispatcher runs cannot see the variable at all — which
# is the property that actually matters and is not implied by the notice being printed.
. "$ROOT/test/lib.sh"

RUNNER="$ROOT/scripts/ci/run-plan"
expect_file "$RUNNER"

# A plan of one gate whose command reports what the environment looked like when it ran.
# `--plan` takes the dispatcher's own model, so nothing here depends on ci-plan's selection.
PLAN="$T/plan.json"
cat > "$PLAN" <<'JSON'
{
  "mode": "case",
  "reason": "one probe gate, to read the environment the dispatcher hands a gate",
  "selected": ["probe"],
  "gates": [
    { "id": "probe", "job": "probe", "selected": true,
      "runs": "printf 'PROBE[%s]\\n' \"${MAJORDOMUS_SHARE:-unset}\"" }
  ]
}
JSON

FOREIGN="$T/some-other-checkout/share"
mkdir -p "$FOREIGN"

# ------------------------------------------------ a share naming another checkout is cleared
out="$T/foreign.txt"
MAJORDOMUS_SHARE="$FOREIGN" bash "$RUNNER" --plan "$PLAN" --job probe > "$out" 2>&1 || true
expect_grep "MAJORDOMUS_SHARE named" "$out"
# the property that matters, and it is not implied by the notice: the gate saw nothing
expect_grep "PROBE\[unset\]" "$out"
expect_no_grep "PROBE\[.*some-other-checkout" "$out"

# ------------------------------------- this checkout's own share passes without a complaint
out="$T/own.txt"
MAJORDOMUS_SHARE="$ROOT/share" bash "$RUNNER" --plan "$PLAN" --job probe > "$out" 2>&1 || true
expect_no_grep "MAJORDOMUS_SHARE named" "$out"

# ------------------------------------------------------------- MJ_KEEP_SHARE=1 keeps it
out="$T/kept.txt"
MJ_KEEP_SHARE=1 MAJORDOMUS_SHARE="$FOREIGN" bash "$RUNNER" --plan "$PLAN" --job probe > "$out" 2>&1 || true
expect_no_grep "MAJORDOMUS_SHARE named" "$out"
expect_grep "PROBE\[.*some-other-checkout" "$out"
