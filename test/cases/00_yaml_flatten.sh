# The YAML subset parser: the foundation everything else validates through.
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
