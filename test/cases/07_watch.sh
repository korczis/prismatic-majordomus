. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null
expect_exit 11 "$MJ" watch
expect_grep 'DRIFT policy .* no projections generated yet'
"$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base
expect_exit 0 "$MJ" watch
expect_grep 'watch: 0 drift'
# policy edited after update
sed -i.bak 's/checkpoint_interval_default: 15m/checkpoint_interval_default: 25m/' .majordomus/policy.yaml; rm -f .majordomus/policy.yaml.bak
expect_exit 11 "$MJ" watch
expect_grep 'DRIFT policy .* changed after the last update'
"$MJ" update >/dev/null
expect_exit 0 "$MJ" watch
# projection hand-edited
echo extra >> AGENTS.md
expect_exit 11 "$MJ" watch
expect_grep 'DRIFT projection +AGENTS.md'
"$MJ" update --force >/dev/null
# active task: scope and staleness
"$MJ" start "t" --scope lib >/dev/null
expect_exit 0 "$MJ" watch
echo x > outside.txt
expect_exit 11 "$MJ" watch
expect_grep 'DRIFT scope +outside.txt'
rm outside.txt
sed -i.bak 's/^checkpoint_at: .*/checkpoint_at: 2020-01-01T00:00:00Z/' .majordomus/state/current.yaml; rm -f .majordomus/state/current.yaml.bak
expect_exit 11 "$MJ" watch
expect_grep 'DRIFT checkpoint'
# completed without a finish record
sed -i.bak 's/^outcome: active/outcome: completed/' .majordomus/state/current.yaml; rm -f .majordomus/state/current.yaml.bak
expect_exit 11 "$MJ" watch
expect_grep 'DRIFT verification .* no task.finished record'
# handed_over without a handover file
sed -i.bak 's/^outcome: completed/outcome: handed_over/' .majordomus/state/current.yaml; rm -f .majordomus/state/current.yaml.bak
expect_exit 11 "$MJ" watch
expect_grep 'DRIFT handover .* no handover file'
# retention cap
sed -i.bak 's/retention_max_lines: 5000/retention_max_lines: 1/' .majordomus/policy.yaml; rm -f .majordomus/policy.yaml.bak
"$MJ" update >/dev/null
expect_exit 11 "$MJ" watch
expect_grep 'DRIFT retention +ledger'
