# A feature exists only when every dimension of the delivery invariant passes (ADR 0071), and
# every dimension is computed: from git, and from the identity document the public site
# serves. This case drives the executable in a fixture repository with a fixture site, so
# nothing here reaches the network: MAJORDOMUS_DELIVERY_SITE_URL points at a file:// directory
# holding a build.json, which is exactly what the site serves at its root.
#
# What it proves, in order:
#   1  a feature whose implementation is merged to origin/master, with a served identity that
#      contains that revision, is on master and DEPLOYED
#   2  it still does not EXIST: the evidence dimensions are unknown in phase 1, unknown is not
#      pass, and its development stage is on_master — a different state from delivered
#   3  negative: the same feature against an OLDER served identity is NOT deployed, and so not
#      publicly verified either
#   4  negative: a change to its implementation on a branch only is implemented_on_branch,
#      not on master
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
export MAJORDOMUS_SHARE="$ROOT/share"
W="$(mktemp -d "${TMPDIR:-/tmp}/mj373.XXXXXX")"; trap 'rm -rf "$W"' EXIT

show() {         # show <name> — `delivery show shipped` as JSON into $W/<name>.json
  run_quiet "$W/$1.err" "$RB" delivery show shipped --format json > "$W/$1.json"
}
jqe() {          # jqe <name> <filter> <what broke>
  jq -e "$2" "$W/$1.json" >/dev/null 2>&1 || { printf '    %s\n' "$3"; jq -c . "$W/$1.json" | head -c 2000; echo; return 1; }
}
site() {         # site <commit> — the public identity the fixture site serves
  mkdir -p "$W/site"
  printf '{"schema":1,"commit":"%s","dirty":false}\n' "$1" > "$W/site/build.json"
}

# ---------------------------------------------------------------- the fixture
"$MJ" init >/dev/null
git checkout -q -B master
mkdir -p .ai/repo/features .ai/repo/ci
cat > .ai/repo/ci/pages.yaml <<'YAML'
version: 1
deploy:
  branch: gh-pages
  identity: build.json
YAML
# Verification is `scripts/pages`' own and is reused rather than re-derived, so a fixture
# repository that has no such script cannot verify a publication and the dimension is
# unknown — which is why one stands here: a stub answering `verify` and `built` the way the
# real script answers them for a site that serves what it should. Without it this case would
# assert the unknown path twice and never the verified one.
mkdir -p scripts
cat > scripts/pages <<'SH'
#!/bin/sh
case "$1" in verify) exit 0;; built) exit 0;; esac
exit 2
SH
chmod +x scripts/pages
cat > .ai/repo/features/shipped.md <<'MD'
---
schema: feature/v1
id: shipped
kind: feature
title: 'The feature this case ships'
headline: 'It is merged, and the site serves it.'
summary: 'A feature made of one capability module, whose source file this fixture commits.'
status: draft
modules: [delivery]
---

## What it does

Ships.
MD
git add -A >/dev/null && git commit -qm install
OLD="$(git rev-parse HEAD)"
mkdir -p apps/majordomus-cli/src/capability/builtin
echo '// the module' > apps/majordomus-cli/src/capability/builtin/delivery.rs
git add -A >/dev/null && git commit -qm implement
MERGED="$(git rev-parse HEAD)"
git update-ref refs/remotes/origin/master HEAD
# what the remote says its default branch is, as a clone records it: the trunk is asked of
# the remote first, because a checkout's own trunk and the remote's default are not the same
# fact and this fixture's identity would otherwise name a branch the clone does not hold
git symbolic-ref refs/remotes/origin/HEAD refs/remotes/origin/master
# the published branch the model names: GitHub's own build of it is what `scripts/pages
# built` reads, and a clone without the ref cannot ask, which is unknown and not a pass
git update-ref refs/remotes/origin/gh-pages HEAD
export MAJORDOMUS_DELIVERY_SITE_URL="file://$W/site"

# ---------------------------------------------------------------- 1, 2  merged and served
site "$MERGED"
show served
jqe served '.implementation.paths == ["apps/majordomus-cli/src/capability/builtin/delivery.rs"]' \
  "the implementation paths are not derived from the module the feature names"
jqe served '.dimensions[0].dimension == "on_master" and .dimensions[0].verdict == "pass"' \
  "a feature merged to origin/master is not on master"
jqe served ".implementation.master_revision == \"$MERGED\"" "the trunk revision is not the merge"
jqe served '.dimensions[1].dimension == "deployed" and .dimensions[1].verdict == "pass"' \
  "a served identity containing the merge is not deployed"
jqe served '[.dimensions[3,4,5] | .verdict] == ["unknown","unknown","unknown"]' \
  "the evidence dimensions are not unknown in phase 1"
jqe served '.dimensions[3].reason | test("PR #577")' "the pending dimensions do not say why"
jqe served '.exists == false and .delivery.state == "not_delivered" and .delivery.stage == "on_master"' \
  "a feature with unknown dimensions exists, or its stage is not on_master"
jqe served '.delivery.blocking == ["required_tests_current","test_evidence_published","ui_linked"]' \
  "blocking does not name exactly the dimensions that do not pass"

# the whole report agrees with the single feature, and orders by id
run_quiet "$W/report.err" "$RB" delivery report --format json > "$W/report.json"
jqe report '.tallies == {"features":1,"on_master":1,"deployed":1,"publicly_verified":1,"exists":0}' \
  "the tallies do not count what the dimensions say"
jqe report '.publication.identity.state == "served" and .trunk == "origin/master"' \
  "the report does not carry the publication it measured against"
expect_exit 0 "$RB" delivery show shipped
expect_grep 'NOT DELIVERED \(on_master\)'

# ---------------------------------------------------------------- 3  an older identity
site "$OLD"
show older
jqe older '.dimensions[1].verdict == "fail"' "an identity older than the merge reads as deployed"
jqe older '.dimensions[1].reason | test("does not contain")' "the refusal does not say why"
jqe older '.dimensions[2].verdict == "fail"' "what is not deployed reads as publicly verified"
jqe older '.exists == false' "a feature that is not deployed exists"

# ---------------------------------------------------------------- 4  a change on a branch
site "$MERGED"
git checkout -q -b feature/more
echo '// more' >> apps/majordomus-cli/src/capability/builtin/delivery.rs
git commit -qam more
show branch
jqe branch '.dimensions[0].verdict == "fail" and .delivery.stage == "implemented_on_branch"' \
  "a change only on the branch reads as on master"
jqe branch ".implementation.master_revision == \"$MERGED\" and .dimensions[1].verdict == \"pass\"" \
  "the trunk's own revision is no longer measured against the site"
