# majordomus-covers: none
# The version a landing allocates (ADR 0134), through the real executable, in a disposable
# repository with a real history: `majordomus release next` reads the trunk's manifest and
# both committed registries from git, so this case controls the answer by what it commits.
#
#   an added public capability      the movement is a minor, and the next version is one
#   a fix behind the boundary       nothing moved, and the policy floor still asks a minor
#   a head behind the trunk         what the trunk added since is not a removal the head made
#   a change one exemption covers   held to no floor, so nothing is allocated
#   a file moved into an exempt tree  judged at both paths, so the move is not exempt
#   the record of the trunk's release   lands at that release's number, though it changes more
#   no floor in the policy          a fix behind the boundary allocates nothing
#   an unreadable trunk             refused with exit 12, never a guessed number
#
# The unit tests in apps/majordomus-cli/src/release/landing.rs own the decision table; this
# case owns the seam they cannot reach: that the command reads the policy file, the refs and
# the registries a real repository holds, and says why in words a reviewer reads.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
command -v jq >/dev/null 2>&1 || { echo "    jq is not installed"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install

# ---------------------------------------------------------------- the trunk
mkdir -p apps/majordomus-cli docs/generated
printf '[package]\nname = "fixture"\nversion = "0.4.0"\nedition = "2021"\n' > apps/majordomus-cli/Cargo.toml
cp "$ROOT/docs/generated/registry.json" docs/generated/registry.json
release_policy() {  # release_policy FLOOR
  awk '/^release:/{skip=1; next} skip && /^[^ ]/{skip=0} !skip' .ai/repo/policy.yaml > policy.tmp
  mv policy.tmp .ai/repo/policy.yaml
  cat >> .ai/repo/policy.yaml <<Y

release:
  allocation: landing
  floor: $1
  exempt:
    - id: plan-records
      description: Plan records, which change no product behaviour
      paths: [".ai/repo/project/**"]
Y
}
release_policy minor
git add -A >/dev/null && git commit -qm trunk
git branch trunk

next() {  # next BASE HEAD -> the decision as JSON on stdout
  "$RB" release next --base "$1" --head "$2" --format json 2>/dev/null
}

# ---------------------------------------------------------------- an added public capability
git checkout -q -b added trunk
jq '.capabilities += [(.capabilities[] | select(.id == "release.analysis")
     | .id = "fixture.added" | .exposure.http.path = "/api/v1/fixture/added"
     | .exposure.mcp.tool = "majordomus_fixture_added")]' \
  docs/generated/registry.json > registry.tmp && mv registry.tmp docs/generated/registry.json
git add -A >/dev/null && git commit -qm "feat: an added capability"
d="$(next trunk added)" || { echo "    release next refused a readable landing"; exit 1; }
[ "$(printf '%s' "$d" | jq -r '[.observed, .selected, .next_version] | join(" ")')" = "minor minor 0.5.0" ] \
  || { echo "    an added capability should be a minor to 0.5.0:"; printf '%s\n' "$d" | sed 's/^/    | /'; exit 1; }

# ---------------------------------------------------------------- a fix behind the boundary
git checkout -q -b fixed trunk
printf 'fn main() {}\n' > apps/majordomus-cli/main.rs
git add -A >/dev/null && git commit -qm "fix: behind the boundary"
d="$(next trunk fixed)"
[ "$(printf '%s' "$d" | jq -r '[.observed, .floor, .next_version] | join(" ")')" = "none minor 0.5.0" ] \
  || { echo "    the floor should raise an unmoved contract to 0.5.0:"; printf '%s\n' "$d" | sed 's/^/    | /'; exit 1; }
text="$("$RB" release next --base trunk --head fixed 2>/dev/null)"
printf '%s' "$text" | grep -q 'the policy floor is minor' \
  || { echo "    the explanation does not name the floor:"; printf '%s\n' "$text" | sed 's/^/    | /'; exit 1; }
printf '%s' "$text" | grep -q '^next  0.5.0$' \
  || { echo "    the text rendering does not end with the next version"; exit 1; }

# ---------------------------------------------------------------- a head behind the trunk
# the trunk gains a capability after `fixed` forked: measured against the trunk itself,
# `fixed` would seem to remove it, so the movement is measured from where it forked
git checkout -q trunk
jq '.capabilities += [(.capabilities[] | select(.id == "release.analysis")
     | .id = "fixture.later" | .exposure.http.path = "/api/v1/fixture/later"
     | .exposure.mcp.tool = "majordomus_fixture_later")]' \
  docs/generated/registry.json > registry.tmp && mv registry.tmp docs/generated/registry.json
git add -A >/dev/null && git commit -qm "feat: the trunk moves on"
d="$(next trunk fixed)"
[ "$(printf '%s' "$d" | jq -r '[.observed, .next_version] | join(" ")')" = "none 0.5.0" ] \
  || { echo "    a head behind the trunk was charged for what the trunk added:"; printf '%s\n' "$d" | sed 's/^/    | /'; exit 1; }

# ---------------------------------------------------------------- a change one exemption covers
git checkout -q -b planned trunk
mkdir -p .ai/repo/project && printf 'id: note\n' > .ai/repo/project/note.yaml
git add -A >/dev/null && git commit -qm "chore: a plan record"
d="$(next trunk planned)"
[ "$(printf '%s' "$d" | jq -r '[.exemption.id, .floor, .next_version] | join(" ")')" = "plan-records none 0.4.0" ] \
  || { echo "    a plan record alone should be exempt and allocate nothing:"; printf '%s\n' "$d" | sed 's/^/    | /'; exit 1; }

# ---------------------------------------------------------------- a file moved into an exempt tree
# with rename detection the move is listed only at its new path, inside the plan records, and
# the change would pass as exempt while it removes a file from the product
git checkout -q -b moved trunk
mkdir -p .ai/repo/project && git mv apps/majordomus-cli/Cargo.toml .ai/repo/project/Cargo.toml
git commit -qm "chore: a move into the plan records"
d="$(next trunk moved)"
[ "$(printf '%s' "$d" | jq -r '[(.exemption // {id: "none"}).id, .floor] | join(" ")')" = "none minor" ] \
  || { echo "    a file moved into an exempt tree passed as exempt:"; printf '%s\n' "$d" | sed 's/^/    | /'; exit 1; }

# ---------------------------------------------------------------- the record of the trunk's release
# a release's record follows its tag in a pull request that may change more than the record
# class covers; held to the floor it would be raised past the tag it records
git checkout -q -b recorded trunk
mkdir -p .ai/repo/releases docs && printf 'version: 0.4.0\n' > .ai/repo/releases/v0.4.0.yaml
printf 'install 0.4.0\n' > docs/INSTALL.md
git add -A >/dev/null && git commit -qm "chore(release): record v0.4.0"
d="$(next trunk recorded)"
[ "$(printf '%s' "$d" | jq -r '[.floor, .next_version] | join(" ")')" = "none 0.4.0" ] \
  || { echo "    the record of the trunk's release was raised past its tag:"; printf '%s\n' "$d" | sed 's/^/    | /'; exit 1; }

# ---------------------------------------------------------------- no floor in the policy
git checkout -q trunk
release_policy none
git add -A >/dev/null && git commit -qm "chore: no floor"
git checkout -q -b quiet trunk
printf 'fn main() {}\n' > apps/majordomus-cli/main.rs
git add -A >/dev/null && git commit -qm "fix: behind the boundary"
d="$(next trunk quiet)"
[ "$(printf '%s' "$d" | jq -r '[.selected, .next_version] | join(" ")')" = "none 0.4.0" ] \
  || { echo "    with no floor a fix behind the boundary should allocate nothing:"; printf '%s\n' "$d" | sed 's/^/    | /'; exit 1; }

# ---------------------------------------------------------------- an unreadable trunk
rc=0; "$RB" release next --base nonesuch --head quiet >/dev/null 2>&1 || rc=$?
[ "$rc" = 12 ] || { echo "    an unreadable trunk should be refused with 12, got $rc"; exit 1; }

echo "    the version a landing allocates: measured, floored, exempted, and refused when unreadable"
