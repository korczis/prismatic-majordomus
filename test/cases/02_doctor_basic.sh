# majordomus-covers: doctor
# majordomus-negative: doctor
# claims: profile-validate, reproduce-command, retention-caps
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

# A profile carrying a key the schema does not know is named, key and file; a policy whose
# default profile has no file fails rather than falling back to another profile.
printf 'bogus_key: 1\n' >> .ai/repo/profiles/routine.yaml
expect_exit 10 "$MJ" doctor
expect_grep 'FAIL profiles +routine — unknown keys: bogus_key'
reset_policy
rm .ai/repo/profiles/implementation.yaml
expect_exit 10 "$MJ" doctor
expect_grep "FAIL profiles +default — profiles.default='implementation' has no file"
reset_policy

# Every failing finding carries the command that reproduces it: in JSON no FAIL has an
# empty reproduce field, and in text every FAIL line ends with one.
expect_exit 10 "$MJ" --json doctor
expect_grep '"level":"FAIL"'
expect_no_grep '"level":"(FAIL|DRIFT)".*"reproduce":""\}'
expect_exit 10 "$MJ" doctor
expect_grep '^FAIL '
expect_no_grep '^(FAIL|DRIFT) .*[^]]$'

# The ledger and the handover store have caps doctor checks: a cap below the count fails,
# naming the count and the cap.
sed -i.bak -e 's/retention_max_lines: 5000/retention_max_lines: 0/' \
  -e 's/retention_max_files: 200/retention_max_files: 0/' .ai/repo/policy.yaml
rm -f .ai/repo/policy.yaml.bak
mkdir -p .ai/local/state/handovers
printf '# Objective\nx\n' > .ai/local/state/handovers/planted.md
expect_exit 10 "$MJ" doctor
expect_grep 'FAIL retention +ledger — [1-9][0-9]* lines over cap 0'
expect_grep 'FAIL retention +handovers — 1 files over cap 0'
rm -f .ai/local/state/handovers/planted.md
reset_policy
expect_exit 10 "$MJ" doctor
expect_grep 'OK +retention +ledger — [0-9]+ lines, cap 5000'
expect_grep 'OK +retention +handovers — 0 files, cap 200'
