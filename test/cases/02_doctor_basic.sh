. "$ROOT/test/lib.sh"
# not installed
expect_exit 12 "$MJ" doctor
expect_grep 'run: majordomus init'
"$MJ" init >/dev/null
# fresh init: nothing wired, no projections -> missing artifact wins
expect_exit 12 "$MJ" doctor
expect_grep 'FAIL wiring +doctor-on-commit'
expect_grep 'FAIL projection +CLAUDE.md — missing'
# unknown key in policy is a failure, named
printf 'nonsense: 1\n' >> .majordomus/policy.yaml
expect_exit 12 "$MJ" doctor
expect_grep 'FAIL policy .*unknown keys: nonsense'
"$MJ" init --force >/dev/null
# malformed policy
printf 'a:\n\tb: 1\n' > .majordomus/policy.yaml
expect_exit 10 "$MJ" doctor
expect_grep 'does not parse'
"$MJ" init --force >/dev/null
# profile name mismatch
sed -i.bak 's/^name: routine/name: other/' .majordomus/profiles/routine.yaml && rm -f .majordomus/profiles/routine.yaml.bak
expect_exit 12 "$MJ" doctor
expect_grep 'FAIL profiles +routine — name field'
# json mode emits one object per finding
"$MJ" init --force >/dev/null
expect_exit 12 "$MJ" --json doctor
expect_grep '^\{"level":"FAIL","category":"wiring"'
