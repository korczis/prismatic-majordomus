# majordomus-covers: plan doctor
# The project model as doctrine: declared in the registry, dispatched by doctor and watch,
# failing the command it runs in, and skipping cleanly where no model exists.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null; "$MJ" update >/dev/null
# init adds the local-state ignore line; commit it so the task that follows starts from a clean tree
git add .gitignore >/dev/null 2>&1; git commit -qm "ignore local ai state" >/dev/null 2>&1 || true
# A fresh checkout has neither declared enforcement hook, and their absence is a real
# doctor failure that would mask the ones this case is about.
mkdir -p .git/hooks
printf '#!/usr/bin/env bash\nmajordomus doctor\n' > .git/hooks/pre-commit
printf '#!/usr/bin/env bash\nmajordomus finish --check\n' > .git/hooks/pre-push
chmod +x .git/hooks/pre-commit .git/hooks/pre-push
PATH="$(dirname "$MJ"):$PATH"; export PATH

# --- both doctrines are declared and both resolve to a validator that exists
expect_exit 0 "$MJ" doctrine show majordomus.project-integrity
expect_grep 'wired       yes'
expect_exit 0 "$MJ" doctrine show majordomus.dag-integrity
expect_grep 'wired       yes'
expect_exit 0 "$MJ" doctrine list
expect_grep '^majordomus.project-integrity +blocking'
expect_grep '^majordomus.dag-integrity +blocking'

# --- a repository with no canonical model is healthy, not red: the model is opt-in
expect_exit 0 "$MJ" doctor
expect_grep 'no canonical project model here'
expect_exit 0 "$MJ" watch

# --- a valid model passes both, and says what it validated
pj_init
pj_milestone M000
pj_issue I0001 M000
pj_issue I0002 M000 I0001
expect_exit 0 "$MJ" doctor
expect_grep 'OK   project'
expect_grep 'OK   dag'

# --- a key nobody reads fails doctor, with a reproduce command
printf 'estimate: 3d\n' >> .ai/repo/project/issues/I0001.yaml
expect_exit 10 "$MJ" doctor
expect_grep 'FAIL project     I0001 — unknown keys: estimate'
expect_grep 'majordomus plan validate'
# ... and watch reports the same thing as drift rather than as a failure
expect_exit 11 "$MJ" watch
expect_grep 'DRIFT project'
sed '$d' .ai/repo/project/issues/I0001.yaml > /tmp/i.$$ && mv /tmp/i.$$ .ai/repo/project/issues/I0001.yaml
expect_exit 0 "$MJ" doctor

# --- a cycle fails doctor through majordomus.dag-integrity, and the failure reaches the exit code
pj_issue I0003 M000 I0004
pj_issue I0004 M000 I0003
expect_exit 10 "$MJ" doctor
expect_grep 'FAIL dag'
expect_grep 'dependency cycle'
expect_exit 11 "$MJ" watch
expect_grep 'DRIFT dag'
rm .ai/repo/project/issues/I0003.yaml .ai/repo/project/issues/I0004.yaml
expect_exit 0 "$MJ" doctor

# --- an unparseable canonical file is a load failure, named as one
printf '\tbroken: yes\n' >> .ai/repo/project/issues/I0002.yaml
expect_exit 10 "$MJ" doctor
expect_grep 'the canonical model does not load'
sed '$d' .ai/repo/project/issues/I0002.yaml > /tmp/i.$$ && mv /tmp/i.$$ .ai/repo/project/issues/I0002.yaml

# --- work in progress is reported, never blocked: an active issue is not a doctor failure
"$MJ" plan start I0001 >/dev/null
expect_exit 0 "$MJ" doctor
expect_grep 'OK   dag'
