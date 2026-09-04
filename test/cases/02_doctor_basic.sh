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
printf 'nonsense: 1\n' >> .ai/repo/policy.yaml
expect_exit 12 "$MJ" doctor
expect_grep 'FAIL policy .*unknown keys: nonsense'
reset_policy
# malformed policy
printf 'a:\n\tb: 1\n' > .ai/repo/policy.yaml
expect_exit 10 "$MJ" doctor
expect_grep 'does not parse'
reset_policy
# profile name mismatch
sed -i.bak 's/^name: routine/name: other/' .ai/repo/profiles/routine.yaml && rm -f .ai/repo/profiles/routine.yaml.bak
expect_exit 12 "$MJ" doctor
expect_grep 'FAIL profiles +routine — name field'
# json mode emits one object per finding
reset_policy
expect_exit 12 "$MJ" --json doctor
expect_grep '^\{"level":"FAIL","category":"wiring"'

# An installation whose policy predates a key must be told which key, not shown a shell
# error. mj_pol_req is used inside command substitutions, where mj_die can only exit the
# subshell — the parent then carried on with an empty value and produced
# "[: : integer expected" plus a finding reading "over budget " with no number. Found by
# running doctor in a repository that adopted the tool before these keys existed.
reset_policy; "$MJ" update >/dev/null
python3 - .ai/repo/policy.yaml <<'PY'
import sys,re
p=sys.argv[1]; s=open(p).read()
s=re.sub(r'^\s*builder_budget_lines:.*\n', '', s, flags=re.M)
s=re.sub(r'^\s*retention_max_files:\s*\d+\s*\n(?=\s*$|\S)', '', s, count=0, flags=re.M)
open(p,'w').write(s)
PY
expect_exit 10 "$MJ" doctor
expect_grep "policy declares no context.builder_budget_lines"
expect_no_grep 'integer expected'
expect_no_grep 'over budget *$'
reset_policy
