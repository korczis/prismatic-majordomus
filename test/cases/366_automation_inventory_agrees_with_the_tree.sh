# majordomus-covers: none
# The tracked automation inventory and this checkout's tree agree: every shell unit under a
# governed directory carries an exemption, and no record names a unit that is gone.
#
# Case 365 proves the check can fail; this one asks it of the repository itself, so a
# branch that adds a script, or deletes one and keeps its record, is red in the suite as
# well as in the structure job's `shell-inventory` gate.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

echo "  this checkout's shell units are all declared, and every record is current"
expect_exit 0 "$RB" shell check --repo "$ROOT"
expect_grep 'shell check: clean'
expect_grep 'exemptions by disposition: A [0-9]+'
