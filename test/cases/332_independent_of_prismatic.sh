# majordomus-covers: none
# `project.independent-of-prismatic`, proved by mutation.
#
# The rule says Majordomus builds, runs and deploys with no Prismatic present. The gate decides
# the part a tree can show, and a gate is only worth the mutation it survives, so the shape of
# this case is: measure a clean tree, add one reference, watch the gate name it, take it away,
# watch the gate go quiet.
#
# What it proves, in order:
#   1  a clean tree passes, and the verdict names the population it measured
#   2  a dependency atom in a manifest is named, with its file and line
#   3  a module call in a library is named, and the same words in lower case are not a module
#   4  a submodule declaration and a gitlink pointing at the Prismatic repository are named
#   5  the project's own name, in every spelling, and the bare brand word are not references
#   6  a document is out of scope: provenance is history, not a dependency
#   7  what is not committed is not measured, and the same line committed is
#   8  a tree that is not a git work tree is unusable, never clean
. "$ROOT/test/lib.sh"

PI="$ROOT/scripts/ci/prismatic-independence-check"
[ -x "$PI" ] || { echo "    scripts/ci/prismatic-independence-check is not executable"; exit 1; }
# the gate measures whatever tree MJ_ROOT names; the fixture is this one
export MJ_ROOT="$PWD"

LIB='#!/usr/bin/env bash\n# a library of Prismatic Majordomus\n'
mkdir -p lib apps/tool docs
printf '%b' "$LIB" > lib/alpha.sh
printf '[package]\nname = "prismatic-majordomus"\nrepository = "https://github.com/korczis/prismatic-majordomus"\n' \
  > apps/tool/Cargo.toml
git add -A >/dev/null && git commit -qm fixture

# ---------------------------------------------------------------- 1. clean
# The verdict states its subject: a pass that did not say how much it looked at is
# indistinguishable from a pass that looked at nothing.
expect_exit 0 "$PI"
expect_grep 'no file names a Prismatic project \(2 file\(s\) measured, 0 submodule declaration\(s\) and 0 gitlink\(s\) examined\)'

# ---------------------------------------------------------------- 2. a dependency atom
printf 'defp deps, do: [{:prismatic, path: "../core"}]\n' > apps/tool/mix.exs
git add -A >/dev/null && git commit -qm atom
expect_exit 10 "$PI"
expect_grep 'FAIL prismatic-independence apps/tool/mix\.exs:1 +names a Prismatic project: `:prismatic'
expect_grep 'reproduce: scripts/ci/prismatic-independence-check'
git rm -q apps/tool/mix.exs && git commit -qm unatom
expect_exit 0 "$PI"

# ---------------------------------------------------------------- 3. a module call
printf '#!/usr/bin/env bash\n# calls Prismatic.Milestones.load over rpc\n' > lib/alpha.sh
git add -A >/dev/null && git commit -qm module
expect_exit 10 "$PI"
expect_grep 'FAIL prismatic-independence lib/alpha\.sh:2 .*`Prismatic\.M'
# the case of the letters is the evidence: the same words in prose are not a module
printf '#!/usr/bin/env bash\n# the prismatic milestones were the model for this\n' > lib/alpha.sh
git add -A >/dev/null && git commit -qm prose
expect_exit 0 "$PI"
printf '%b' "$LIB" > lib/alpha.sh
git add -A >/dev/null && git commit -qm restore

# ---------------------------------------------------------------- 4. submodules
printf '[submodule "vendor/core"]\n\tpath = vendor/core\n\turl = git@gitlab.com:korczis/prismatic-platform.git\n' > .gitmodules
git add .gitmodules && git commit -qm submodule
expect_exit 10 "$PI"
expect_grep 'FAIL prismatic-independence \.gitmodules .*prismatic-platform'
expect_grep '1 submodule declaration\(s\) and 0 gitlink\(s\) examined'
git rm -q .gitmodules && git commit -qm unsubmodule
expect_exit 0 "$PI"

git update-index --add --cacheinfo "160000,$(git rev-parse HEAD),vendor/prismatic_web"
git commit -qm gitlink
expect_exit 10 "$PI"
expect_grep 'FAIL prismatic-independence vendor/prismatic_web .*a gitlink to vendor/prismatic_web'
expect_grep '0 submodule declaration\(s\) and 1 gitlink\(s\) examined'
git rm -q --cached vendor/prismatic_web && git commit -qm ungitlink
expect_exit 0 "$PI"

# ---------------------------------------------------------------- 5. the project's own name
# Naming this repository, in any of its spellings, is not naming another, and the brand word
# on its own is the one name project.clean-room allows. A gate that flagged them would be red
# on the day it was written and turned off the day after.
printf '#!/usr/bin/env bash\n# PRISMATIC, Prismatic Majordomus, prismatic_majordomus, github.com/korczis/prismatic-majordomus\n' > lib/alpha.sh
git add -A >/dev/null && git commit -qm identity
expect_exit 0 "$PI"
expect_no_grep 'FAIL'
printf '%b' "$LIB" > lib/alpha.sh
git add -A >/dev/null && git commit -qm restore-identity

# ---------------------------------------------------------------- 6. documents
# A design note may record where an idea came from; that is provenance, and the rule allows it.
printf '# Provenance\n\nInspired by the prismatic-platform session model (Prismatic.Session); implemented here.\n' > docs/DESIGN.md
git add -A >/dev/null && git commit -qm provenance
expect_exit 0 "$PI"
expect_no_grep 'docs/DESIGN'

# ---------------------------------------------------------------- 7. only what is committed
printf 'url = /srv/prismatic.git\n' > lib/remote.conf
expect_exit 0 "$PI"
# the positive control: the same line committed is a finding, so the silence above was the
# scope and not a pattern that cannot match
git add lib/remote.conf && git commit -qm remote
expect_exit 10 "$PI"
expect_grep 'FAIL prismatic-independence lib/remote\.conf:1 .*`/prismatic\.git'
git rm -q lib/remote.conf && git commit -qm unremote
expect_exit 0 "$PI"

# ---------------------------------------------------------------- 8. not a repository
NOGIT="$(mktemp -d)"
if git -C "$NOGIT" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  echo "    $NOGIT is inside a git work tree; this case cannot build a non-repository"
  rm -rf "$NOGIT"
  exit 1
fi
expect_exit 12 env MJ_ROOT="$NOGIT" "$PI"
expect_grep 'is not a git work tree; nothing to measure'
rm -rf "$NOGIT"
