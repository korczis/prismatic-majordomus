# The YAML subset parser: the foundation everything else validates through.
# claims: policy-parse
. "$ROOT/test/lib.sh"
. "$ROOT/lib/common.sh"
cat > y.yaml <<'Y'
# comment
version: 1
context:
  budget: 150          # trailing comment
  strategy: "minimum-sufficient"
profiles:
  default: implementation
scope:
  - lib/auth
  - "docs/"
same_indent:
- a
- b
enforcement:
  - name: first
    path: bin/x
    args: [doctor, --strict]
    nested:
      - deep
  - name: second
    args: []
empty_list: []
Y
mj_yaml_flatten y.yaml > flat.txt
expect_grep '^version=1$' flat.txt
expect_grep '^context\.budget=150$' flat.txt
expect_grep '^context\.strategy=minimum-sufficient$' flat.txt
expect_grep '^scope\.0=lib/auth$' flat.txt
expect_grep '^scope\.1=docs/$' flat.txt
expect_grep '^same_indent\.1=b$' flat.txt
expect_grep '^enforcement\.0\.name=first$' flat.txt
expect_grep '^enforcement\.0\.path=bin/x$' flat.txt
expect_grep '^enforcement\.0\.args\.1=--strict$' flat.txt
expect_grep '^enforcement\.0\.nested\.0=deep$' flat.txt
expect_grep '^enforcement\.1\.name=second$' flat.txt
expect_grep '^enforcement\.1\.args=\[\]$' flat.txt
expect_grep '^empty_list=\[\]$' flat.txt
[ "$(mj_yget flat.txt context.budget)" = 150 ]
# inside single quotes a doubled quote is one quote, as every YAML parser reads it; a
# double-quoted scalar has no such escape and keeps the pair. The Rust engine reads the
# same (metadata::yaml, a_doubled_quote_inside_single_quotes_is_one_quote).
printf '%s\n' "a: 'the repository''s test'" 'b: "it'"''"'s"' "c: ''''" "d: ['x''y', z]" > q.yaml
mj_yaml_flatten q.yaml > qf.txt
expect_grep "^a=the repository's test$" qf.txt
expect_grep "^b=it''s$" qf.txt
expect_grep "^c='$" qf.txt
expect_grep "^d\.0=x'y$" qf.txt
expect_grep '^d\.1=z$' qf.txt
# inside double quotes an escaped quote, backslash or slash reads as the character, in one
# pass, so an escaped backslash before a quote stays one backslash (I3011); an escape a line
# cannot hold (a newline) stands as written. The Rust engine reads the same
# (metadata::yaml, a_double_quoted_scalar_reads_its_escapes).
cat > e.yaml <<'E'
a: "echo \"held\""
b: "a \\ b \/ c"
c: "one\ntwo"
d: "ends in \\"
E
mj_yaml_flatten e.yaml > ef.txt
[ "$(mj_yget ef.txt a)" = 'echo "held"' ] || { echo "    a: $(mj_yget ef.txt a)"; exit 1; }
[ "$(mj_yget ef.txt b)" = 'a \ b / c' ] || { echo "    b: $(mj_yget ef.txt b)"; exit 1; }
[ "$(mj_yget ef.txt c)" = 'one\ntwo' ] || { echo "    c: $(mj_yget ef.txt c)"; exit 1; }
[ "$(mj_yget ef.txt d)" = 'ends in \' ] || { echo "    d: $(mj_yget ef.txt d)"; exit 1; }
[ "$(mj_ylist flat.txt scope | wc -l | tr -d ' ')" = 2 ]
# failures
printf 'a:\n\tb: 1\n' > tab.yaml
expect_exit 3 mj_yaml_flatten tab.yaml
printf -- '- orphan\n' > orphan.yaml
expect_exit 3 mj_yaml_flatten orphan.yaml
printf 'weird line without colon\n' > bad.yaml
expect_exit 3 mj_yaml_flatten bad.yaml
# unknown-key detection
printf '^version$\n^context\\.budget$\n' > allow.txt
printf 'version: 1\ncontext:\n  budget: 1\n  extra: 2\n' > k.yaml
mj_yaml_flatten k.yaml > kf.txt
out="$(mj_yaml_unknown_keys kf.txt allow.txt)" && exit 1
[ "$out" = "context.extra" ]
# the canonical policy is parsed through this subset, and a key its allowlist does not name
# rejects it by name (claim policy-parse)
"$MJ" init >/dev/null
expect_exit 12 "$MJ" doctor
expect_grep 'OK +policy +\.ai/repo/policy\.yaml — parsed, version 1'
printf 'nonsense: 1\n' >> .ai/repo/policy.yaml
expect_exit 12 "$MJ" doctor
expect_grep 'FAIL policy .*unknown keys: nonsense'
