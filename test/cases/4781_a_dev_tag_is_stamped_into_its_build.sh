# majordomus-covers: none
# A dev tag is released by the pipeline a stable tag goes through, with its version stamped
# into the build and never into the tree (ADR 0240, I4811).
#
# scripts/ci/release-autotag tags a landed commit vX.Y.Z-dev.N where the commit declares X.Y.Z.
# release.yml's first check refused that: the tag had to equal the declared version.
# scripts/release-stamp is the smallest step that lets the same job release it:
#
#   1  --check over a stable tag is release-version --check --tag: the declared tag passes,
#      any other is refused, as before
#   2  --check over a dev tag passes for v<declared>-dev.<N> and refuses another base, a
#      number that is not a positive whole number, and a tree that declares a pre-release
#   3  the stamp writes the manifest's [package] version, the lock's own entry and
#      share/version.txt, touches no dependency's version, and afterwards the shell tool
#      prints the dev version and release-version --check --tag passes; a stable tag stamps
#      nothing
#   4  release.yml asks release-stamp --check in its plan job and stamps the build job's
#      checkout before the package is built; release-auto.yml runs after a successful master
#      validate and runs release-autotag --apply; its model declares the switch, off or
#      automatic and nothing else
#   5  a dev release is a pre-release on every surface that names one: site-deploy,
#      install-check, release-check and release.yml's smoke share the pattern, and
#      release-check orders alpha < beta < dev < rc below the final release
. "$ROOT/test/lib.sh"
unset MJ_ROOT

P="$T/p"
mkdir -p "$P/scripts/ci" "$P/apps/majordomus-cli" "$P/share" "$P/.ai/repo/releases"
cp "$ROOT/scripts/release-stamp" "$ROOT/scripts/release-version" "$P/scripts/"
cp -R "$ROOT/bin" "$ROOT/lib" "$P/"
cat > "$P/apps/majordomus-cli/Cargo.toml" <<'T'
[package]
name = "majordomus-cli"
version = "4.2.0"
edition = "2021"

[dependencies]
serde = { version = "4.2.0" }
T
cat > "$P/apps/majordomus-cli/Cargo.lock" <<'T'
version = 4

[[package]]
name = "majordomus-cli"
version = "4.2.0"
dependencies = [
 "serde",
]

[[package]]
name = "serde"
version = "4.2.0"
T
printf '# GENERATED FILE\n# Generator: majordomus-cli 4.2.0\nversion=4.2.0\n' > "$P/share/version.txt"
stamp() { ( cd "$P" && env -u MAJORDOMUS_SHARE ./scripts/release-stamp "$@" ); }

# ---------------------------------------------------------------- 1. a stable tag, as before
expect_exit 0 stamp --check --tag v4.2.0 || { echo "    the declared stable tag was refused"; exit 1; }
expect_grep 'release-version: OK   tag v4.2.0' || exit 1
expect_exit 10 stamp --check --tag v4.3.0 || { echo "    a stable tag the tree does not declare passed"; exit 1; }

# ---------------------------------------------------------------- 2. a dev tag of the declared version
expect_exit 0 stamp --check --tag v4.2.0-dev.7 || { echo "    a dev tag of the declared version was refused"; exit 1; }
expect_grep 'OK   v4.2.0-dev.7 is dev release 7 of the 4.2.0 this tree declares' || exit 1
expect_grep 'release-version: OK   bin/majordomus prints 4.2.0' || exit 1
expect_exit 10 stamp --check --tag v4.3.0-dev.1 || { echo "    a dev tag of another version passed"; exit 1; }
expect_grep 'is a dev release of 4.3.0 and this tree declares 4.2.0' || exit 1
for bad in v4.2.0-dev.0 v4.2.0-dev.x v4.2.0-dev.07; do
  expect_exit 10 stamp --check --tag "$bad" || { echo "    $bad passed"; exit 1; }
  expect_grep 'the dev number must be a positive whole number' || exit 1
done
expect_exit 2 stamp --check || exit 1
expect_exit 2 stamp --tag v4.2.0-dev.1 --sideways || exit 1

# ---------------------------------------------------------------- 3. the stamp
expect_exit 0 stamp --tag v4.2.0 || exit 1
expect_grep 'is not a dev tag; .* nothing stamped' || exit 1
grep -q '^version = "4.2.0"$' "$P/apps/majordomus-cli/Cargo.toml" || { echo "    a stable tag stamped the manifest"; exit 1; }
expect_exit 0 stamp --tag v4.2.0-dev.7 || { echo "    the stamp failed"; exit 1; }
expect_grep '4.2.0-dev.7 stamped into this checkout .*never commit it' || exit 1
expect_grep 'release-version: OK   tag v4.2.0-dev.7' || { echo "    release-version does not accept the stamped tree"; exit 1; }
[ "$(grep -c '^version = "4.2.0-dev.7"$' "$P/apps/majordomus-cli/Cargo.toml")" = 1 ] \
  || { echo "    the manifest was not stamped once"; cat "$P/apps/majordomus-cli/Cargo.toml"; exit 1; }
grep -q 'serde = { version = "4.2.0" }' "$P/apps/majordomus-cli/Cargo.toml" || { echo "    a dependency's version in the manifest moved"; exit 1; }
[ "$(awk '/^name = "majordomus-cli"$/{getline; print}' "$P/apps/majordomus-cli/Cargo.lock")" = 'version = "4.2.0-dev.7"' ] \
  || { echo "    the lock's own entry was not stamped"; cat "$P/apps/majordomus-cli/Cargo.lock"; exit 1; }
[ "$(awk '/^name = "serde"$/{getline; print}' "$P/apps/majordomus-cli/Cargo.lock")" = 'version = "4.2.0"' ] \
  || { echo "    a dependency's entry in the lock moved"; exit 1; }
grep -q '^# Generator: majordomus-cli 4.2.0$' "$P/share/version.txt" && grep -q '^version=4.2.0-dev.7$' "$P/share/version.txt" \
  || { echo "    share/version.txt was not stamped past its banner"; cat "$P/share/version.txt"; exit 1; }
[ "$(cd "$P" && env -u MAJORDOMUS_SHARE ./bin/majordomus version)" = "majordomus 4.2.0-dev.7" ] \
  || { echo "    the shell tool does not print the stamped version"; exit 1; }
# a stamped tree declares a pre-release, and no dev tag is cut over one
expect_exit 10 stamp --check --tag v4.2.0-dev.7-dev.1 || exit 1
expect_grep 'declares the pre-release 4.2.0-dev.7' || exit 1
sed 's/^version = "4.2.0-dev.7"$/version = "4.2.0"/' "$P/apps/majordomus-cli/Cargo.toml" > "$P/m" && mv "$P/m" "$P/apps/majordomus-cli/Cargo.toml"
rm "$P/share/version.txt"
expect_exit 12 stamp --tag v4.2.0-dev.7 || { echo "    a missing projection was not refused"; exit 1; }

# ---------------------------------------------------------------- 4. the wiring
mj_runs python3 -c 'import yaml' || skip "python3 has no yaml module ($(mj_runs_why python3))"
python3 - "$ROOT" <<'PY' || exit 1
import sys, yaml
root = sys.argv[1]
def load(p):
    return yaml.safe_load(open(f"{root}/{p}", encoding="utf-8"))
rel = load(".github/workflows/release.yml")
plan = "\n".join(str(s.get("run", "")) for s in rel["jobs"]["plan"]["steps"])
if 'scripts/release-stamp --check --tag "$tag"' not in plan:
    print("    release.yml's plan job does not ask release-stamp --check for the tag"); sys.exit(1)
runs = [str(s.get("run", "")) for s in rel["jobs"]["build"]["steps"]]
at = [i for i, r in enumerate(runs) if r.startswith("scripts/release-stamp --tag ")]
pkg = [i for i, r in enumerate(runs) if "scripts/release-package" in r]
if not at or not pkg or at[0] > pkg[0]:
    print("    the build job does not stamp its checkout before it packages"); sys.exit(1)
auto = load(".github/workflows/release-auto.yml")
on = auto.get(True) or auto.get("on")
wr = on.get("workflow_run") or {}
if wr.get("workflows") != ["validate"] or wr.get("branches") != ["master"] or "schedule" not in on:
    print("    release-auto.yml does not follow master's validate, with a schedule behind it"); sys.exit(1)
job = auto["jobs"]["tag"]
cond = job.get("if", "")
for need in ("conclusion == 'success'", "event == 'push'", "head_branch == 'master'"):
    if need not in cond:
        print(f"    release-auto.yml tags without {need}"); sys.exit(1)
if job["permissions"].get("contents") != "write" or job["permissions"].get("actions") != "write":
    print("    release-auto.yml cannot push a tag and start release.yml"); sys.exit(1)
if not any("scripts/ci/release-autotag" in str(s.get("run", "")) and "--apply" in str(s.get("run", "")) for s in job["steps"]):
    print("    release-auto.yml does not run release-autotag --apply"); sys.exit(1)
if load(".ai/repo/ci/release-auto.yaml").get("publication") not in ("off", "automatic"):
    print("    the model does not declare publication as off or automatic"); sys.exit(1)
print("    release.yml stamps a dev tag's build, and release-auto.yml tags a green master")
PY

# ---------------------------------------------------------------- 5. a dev release is a pre-release everywhere
for f in scripts/site-deploy scripts/ci/install-check scripts/ci/release-check .github/workflows/release.yml; do
  grep -q 'alpha|beta|rc|dev' "$ROOT/$f" || { echo "    $f does not take a -dev.N tag for a pre-release"; exit 1; }
done
grep -q -- '-dev\.\[0-9\]\*' "$ROOT/scripts/site-deploy" || { echo "    site-deploy lets latest.json name a dev release"; exit 1; }
# release-check's precedence, over records of each pre-release kind
mkdir -p "$P/apps/majordomus-cli/src/release"
cp "$ROOT/scripts/ci/release-check" "$P/scripts/ci/"
cp "$ROOT/apps/majordomus-cli/src/release/version.rs" "$P/apps/majordomus-cli/src/release/"
order() { # <declared> <record>...: what release-check says of a tree declaring <declared>
  local d="$1" r; shift
  rm -f "$P"/.ai/repo/releases/*.yaml
  for r in "$@"; do printf 'version: "%s"\n' "$r" > "$P/.ai/repo/releases/v$r.yaml"; done
  awk -v v="$d" '/^\[package\]/{p=1} p && /^version *=/ && !x { print "version = \"" v "\""; x = 1; next } { print }' \
    "$P/apps/majordomus-cli/Cargo.toml" > "$P/m" && mv "$P/m" "$P/apps/majordomus-cli/Cargo.toml"
  ( cd "$P" && env -u MAJORDOMUS_SHARE MJ_RELEASE_CHECK_REPO=o/r ./scripts/ci/release-check 2>&1 ) \
    | grep -E 'and the newest release is|is already published' || true
}
got="$(order 4.2.0 4.2.0-dev.9 4.1.0)"
case "$got" in *"the tree declares 4.2.0 and the newest release is 4.2.0-dev.9"*) ;;
  *) echo "    a tree declaring 4.2.0 is not ahead of its dev releases: $got"; exit 1 ;; esac
order 4.2.0 4.2.0-beta.3 4.2.0-dev.2 | grep -q 'newest release is 4.2.0-dev.2' || { echo "    dev does not order above beta"; exit 1; }
order 4.2.0 4.2.0-rc.1 4.2.0-dev.2 | grep -q 'newest release is 4.2.0-rc.1' || { echo "    dev does not order below rc"; exit 1; }
order 4.1.0 4.2.0-dev.1 | grep -q 'the tree declares 4.1.0 and 4.2.0-dev.1 is already published' \
  || { echo "    a tree behind a dev release is not found: $(order 4.1.0 4.2.0-dev.1)"; exit 1; }
exit 0
