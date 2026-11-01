# majordomus-covers: doctor
# majordomus-negative: doctor
. "$ROOT/test/lib.sh"
# share/profile-limits.yaml is the one place that says which profile axis becomes which
# runtime limit (I0201). doctor resolves every profile's limits from it and prints them as
# data, and refuses by name: an axis a profile does not set, a derivation it does not know,
# a declared adapter no function carries, and a function that applies an undeclared limit.
# The last three are faults of the tool, not of the repository, so they are made in a copy
# of the tool, never in $ROOT; scratch copies live outside the repository under test.
W="$(mktemp -d "${TMPDIR:-/tmp}/mj-pl.XXXXXX")"; trap 'rm -rf "$W"' EXIT
doctor_out() { LAST_OUT="$("$1" doctor 2>&1)" || true; }

"$MJ" init >/dev/null

# every shipped profile resolves to a complete set of limits, as name=value
doctor_out "$MJ"
for p in $(for f in .ai/repo/profiles/*.yaml; do basename "$f" .yaml; done); do
  expect_grep "OK +profile-limits +$p — limits checkpoint_max_age_seconds=[0-9]+ history_events=[0-9]+ knowledge_pointers=[0-9]+$"
done
expect_grep "OK +profile-limits +routine — limits checkpoint_max_age_seconds=1800 "
expect_no_grep 'FAIL +profile-limits'

# a profile that does not set a declared axis is named, with the limit it would derive
cp .ai/repo/profiles/debugging.yaml "$W/debugging.yaml"
grep -v 'knowledge_max_items' "$W/debugging.yaml" > .ai/repo/profiles/debugging.yaml
doctor_out "$MJ"
expect_grep 'FAIL +profile-limits +debugging — does not set context.knowledge_max_items, from which share/profile-limits.yaml derives knowledge_pointers'
expect_no_grep 'OK +profile-limits +debugging'
# an axis value the derivation cannot read is refused, not taken as zero
sed 's/^checkpoint_interval: .*/checkpoint_interval: soon/' "$W/debugging.yaml" > .ai/repo/profiles/debugging.yaml
doctor_out "$MJ"
expect_grep "FAIL +profile-limits +debugging — checkpoint_interval: 'soon' is not a duration_seconds value"
cp "$W/debugging.yaml" .ai/repo/profiles/debugging.yaml

# the tool's own declaration, reconciled in both directions, in a copy of the tool
mkdir -p "$W/tool"; cp -R "$ROOT/bin" "$ROOT/lib" "$ROOT/share" "$W/tool/"
TOOL="$W/tool/bin/majordomus"
LIMITS="$W/tool/share/profile-limits.yaml"; cp "$LIMITS" "$W/limits.yaml"

# a declared adapter that no function carries
sed 's/adapter: mj_ctx_history$/adapter: mj_ctx_history_gone/' "$W/limits.yaml" > "$LIMITS"
doctor_out "$TOOL"
expect_grep "FAIL +profile-limits +history_events — declares adapter mj_ctx_history_gone, and no function mj_ctx_history_gone under lib/ carries '# majordomus-limit: history_events'"

# a derivation the file does not define
sed 's/derive: count$/derive: guess/' "$W/limits.yaml" > "$LIMITS"
doctor_out "$TOOL"
expect_grep "FAIL +profile-limits +history_events — derive 'guess' is not count or duration_seconds"
cp "$W/limits.yaml" "$LIMITS"

# a function that applies a limit the file does not declare
printf '\n# majordomus-limit: subagent_budget\nmj_clamp_subagents() {\n  :\n}\n' >> "$W/tool/lib/context.sh"
doctor_out "$TOOL"
expect_grep "FAIL +profile-limits +subagent_budget — .*lib/context.sh: mj_clamp_subagents applies limit 'subagent_budget', which share/profile-limits.yaml does not declare"

# the copy, restored, is clean again: the faults above were the only ones
cp "$ROOT/lib/context.sh" "$W/tool/lib/context.sh"
doctor_out "$TOOL"
expect_no_grep 'FAIL +profile-limits'
