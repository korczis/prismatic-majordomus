# majordomus-covers: doctor
# doctor names a checkout whose dependency install is behind its lockfile (I4595).
#
# On 2026-11-14 an adopter installed and committed a new esbuild in a linked worktree; the
# primary checkout's pre-push then died halfway through its build with ERR_MODULE_NOT_FOUND,
# because that checkout's node_modules predated the new package-lock.json. Nothing in
# Majordomus said so first.
#
# A fixture with three checkouts: the primary, a linked worktree `feature`, and a linked
# worktree `bare` on the commit before any package file, with a node_modules and no lockfile.
# The packages are local (file:vendor/...), so every npm here runs offline.
#
#   both installs current               one OK line; `bare` is not judged
#   a dependency added in `feature`,    doctor run in `feature` warns about the primary by
#   merged into the primary             path, with package-lock.json and `npm ci`
#   `npm ci` in the primary             the warning is gone, the OK line is back
#   the roles swapped                   doctor run in the primary warns about `feature`;
#                                       in the commit context it reads only its own checkout
#                                       and says so
#   a mix project whose deps/ lacks     a mix that does not start makes it INFO, never a
#   what mix.lock holds                 WARN; where mix starts, a WARN with `mix deps.get`
#
# And none of it writes: every checkout's status, every install's record and the mix
# project's tree are the same before and after doctor.
#
# Skips itself when jq is absent or npm does not start here.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq not installed"
. "$ROOT/lib/runnable.sh"
mj_runs npm || skip "npm does not start here: $(mj_runs_why npm)"
W="$(mktemp -d "${TMPDIR:-/tmp}/mj4628.XXXXXX")"; trap 'rm -rf "$W"' EXIT
W="$(cd "$W" && pwd -P)"
P="$(pwd -P)"
NPM_FLAGS="--offline --no-audit --no-fund --ignore-scripts --loglevel=error"

"$MJ" init >/dev/null
printf 'node_modules/\ndeps/\n_build/\n' >> .gitignore
git add -A >/dev/null && git commit -qm "the layer"
layer="$(git rev-parse HEAD)"; trunk="$(git rev-parse --abbrev-ref HEAD)"

pkg() {  # pkg DIR DEP...: a package.json depending on each vendored DEP
  local d="$1" deps="" x; shift
  for x in "$@"; do
    mkdir -p "$d/vendor/$x"
    printf '{"name":"%s","version":"1.0.0"}\n' "$x" > "$d/vendor/$x/package.json"
    deps="${deps:+$deps,}\"$x\":\"file:vendor/$x\""
  done
  printf '{"name":"fx","version":"0.0.0","private":true,"dependencies":{%s}}\n' "$deps" > "$d/package.json"
}
# shellcheck disable=SC2086 # the flags are words
npm_in() { ( cd "$1" && shift && npm "$@" $NPM_FLAGS >/dev/null 2>&1 ) || { echo "    npm $2 failed in $1"; exit 1; }; }
doctor_in() {  # doctor_in DIR OUT [VAR=VALUE...]: doctor's clone lines, run in DIR
  local d="$1" out="$2"; shift 2
  ( cd "$d" && env "$@" "$MJ" doctor ) > "$W/$out.full" 2>&1 || true
  grep -E '^(OK|WARN|INFO|FAIL) +clone ' "$W/$out.full" > "$W/$out" || true
}
bad() { echo "    $1"; cat "$W/$2"; exit 1; }
OK_LINE="OK   clone       install — every checkout's dependency install matches its lockfile"

pkg "$P" dep-a
npm_in "$P" install
git add package.json package-lock.json vendor && git commit -qm "dep-a"
git worktree add -q -b work/feature "$W/feature"
npm_in "$W/feature" ci
git worktree add -q -b work/bare "$W/bare" "$layer"
mkdir -p "$W/bare/node_modules/stray"     # an install with no lockfile: not judged

# ---------------------------------------------------------------- both current
doctor_in "$P" current
grep -qF "$OK_LINE (2 install(s) read)" "$W/current" || bad "two current installs do not read as one OK line:" current
grep -q "$W/bare" "$W/current" && bad "a checkout with no lockfile was judged:" current

# ---------------------------------------------------------------- the primary behind
pkg "$W/feature" dep-a dep-b
npm_in "$W/feature" install
( cd "$W/feature" && git add package.json package-lock.json vendor && git commit -qm "dep-b" )
git merge -q --ff-only work/feature
[ -e node_modules/dep-b ] && { echo "    the fixture is wrong: the primary already has dep-b"; exit 1; }

fingerprint() {
  local d
  for d in "$P" "$W/feature" "$W/bare"; do
    git -C "$d" --no-optional-locks status --porcelain=v1 --untracked-files=all
    if [ -f "$d/node_modules/.package-lock.json" ]; then sha256_of_file "$d/node_modules/.package-lock.json"; fi
    ls -a "$d/node_modules"
  done
  if [ -d "$W/mixed" ]; then (cd "$W/mixed" && find . -path ./.git -prune -o -print | LC_ALL=C sort); fi
}
fingerprint > "$W/before.txt"

doctor_in "$W/feature" primary-behind
grep -qF "WARN clone       $P — package-lock.json is ahead of node_modules: 1 package(s) it locks are missing or at another version (dep-b)" "$W/primary-behind" \
  || bad "doctor run in the worktree does not name the primary checkout behind its lockfile:" primary-behind
grep -qF "[reproduce: cd '$P' && npm ci]" "$W/primary-behind" || bad "the warning does not name npm ci in the primary:" primary-behind
grep -q "WARN clone       $W/feature " "$W/primary-behind" && bad "the current worktree is warned about:" primary-behind
grep -qF "$OK_LINE" "$W/primary-behind" && bad "an OK line beside a checkout that is behind:" primary-behind

fingerprint > "$W/after.txt"
cmp -s "$W/before.txt" "$W/after.txt" \
  || { echo "    doctor changed a checkout or an install:"; diff "$W/before.txt" "$W/after.txt" | head -20; exit 1; }

npm_in "$P" ci
doctor_in "$W/feature" primary-installed
grep -q "WARN clone       $P " "$W/primary-installed" && bad "the warning outlives npm ci:" primary-installed
grep -qF "$OK_LINE (2 install(s) read)" "$W/primary-installed" || bad "after npm ci the OK line is not back:" primary-installed

# ---------------------------------------------------------------- the roles swapped
pkg "$P" dep-a dep-b dep-c
npm_in "$P" install
git add package.json package-lock.json vendor && git commit -qm "dep-c"
( cd "$W/feature" && git merge -q --ff-only "$trunk" )
doctor_in "$P" worktree-behind
grep -qF "WARN clone       $W/feature — package-lock.json is ahead of node_modules: 1 package(s) it locks are missing or at another version (dep-c)" "$W/worktree-behind" \
  || bad "doctor run in the primary does not name the linked worktree behind its lockfile:" worktree-behind
grep -qF "[reproduce: cd '$W/feature' && npm ci]" "$W/worktree-behind" || bad "the warning does not name npm ci in the worktree:" worktree-behind
grep -q "WARN clone       $P " "$W/worktree-behind" && bad "the current primary is warned about:" worktree-behind

# the commit hook reads its own checkout only, and says it left the others out
doctor_in "$P" hook MAJORDOMUS_DOCTOR_CONTEXT=commit
grep -q "$W/feature" "$W/hook" && bad "in the commit context doctor read another checkout:" hook
grep -qF "INFO clone       install — other checkouts' installs are not read in the commit hook (MAJORDOMUS_DOCTOR_CONTEXT=commit), only this one's; run doctor on its own  [reproduce: majordomus doctor]" "$W/hook" \
  || bad "in the commit context doctor does not say it left the other checkouts out:" hook
npm_in "$W/feature" ci

# ---------------------------------------------------------------- mix
git worktree add -q -b work/mixed "$W/mixed" "$layer"
mkdir -p "$W/mixed/deps"
cat > "$W/mixed/mix.exs" <<'EOF'
defmodule Fx.MixProject do
  use Mix.Project
  def project, do: [app: :fx, version: "0.1.0", deps: [{:dep_a, git: "https://example.invalid/dep_a.git", tag: "v1"}]]
end
EOF
printf '%%{\n  "dep_a": {:git, "https://example.invalid/dep_a.git", "0123456789abcdef0123456789abcdef01234567", [tag: "v1"]},\n}\n' > "$W/mixed/mix.lock"

# a mix that does not start: INFO, never a WARN or a failure
mkdir -p "$W/stub"
printf '#!/bin/sh\necho "No version is set for command mix" >&2\nexit 126\n' > "$W/stub/mix"; chmod +x "$W/stub/mix"
doctor_in "$P" mix-absent PATH="$W/stub:$PATH"
grep -qF "INFO clone       $W/mixed — mix.lock: mix is on PATH but does not start (No version is set for command mix)" "$W/mix-absent" \
  || bad "a mix that does not start is not reported as INFO for the mix checkout:" mix-absent
grep -q "WARN clone       $W/mixed " "$W/mix-absent" && bad "a mix that does not start produced a WARN:" mix-absent
grep -qF "$OK_LINE" "$W/mix-absent" && bad "an OK line beside an install that was not read:" mix-absent

# where mix starts: the deps the lock holds and deps/ lacks are named, with mix deps.get
mix_env=""
if ! (cd "$W/mixed" && mj_runs mix) && command -v asdf >/dev/null 2>&1; then
  for ev in $(asdf list elixir 2>/dev/null | tr -d ' *' | grep -- '-otp-'); do
    for erl in $(asdf list erlang 2>/dev/null | tr -d ' *' | grep "^${ev##*-otp-}"); do
      if (cd "$W/mixed" && ASDF_ELIXIR_VERSION="$ev" ASDF_ERLANG_VERSION="$erl" mj_runs mix); then
        mix_env="ASDF_ELIXIR_VERSION=$ev ASDF_ERLANG_VERSION=$erl"; break 2
      fi
    done
  done
fi
# shellcheck disable=SC2086 # the assignments are words
if (cd "$W/mixed" && env $mix_env sh -c 'command -v mix >/dev/null && mix --version >/dev/null 2>&1'); then
  fingerprint > "$W/before.txt"
  # shellcheck disable=SC2086 # the assignments are words
  doctor_in "$P" mix-behind $mix_env
  grep -qF "WARN clone       $W/mixed — mix.lock is ahead of deps/: mix deps.loadpaths --no-compile refuses (dep_a)" "$W/mix-behind" \
    || bad "a mix project whose deps/ lacks a locked dependency is not named:" mix-behind
  grep -qF "[reproduce: cd '$W/mixed' && mix deps.get]" "$W/mix-behind" || bad "the warning does not name mix deps.get:" mix-behind
  fingerprint > "$W/after.txt"
  cmp -s "$W/before.txt" "$W/after.txt" \
    || { echo "    doctor's mix reading changed a checkout:"; diff "$W/before.txt" "$W/after.txt" | head -20; exit 1; }
else
  echo "    note: mix does not start here, so the WARN half of the mix reading is not exercised"
fi
exit 0
