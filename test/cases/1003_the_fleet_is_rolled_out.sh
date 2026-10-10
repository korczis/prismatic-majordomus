# majordomus-covers: none
# majordomus-timeout: 600
# The fleet is rolled out from one terminal (ADR 0121).
#
# A fleet declaration names the machines that run the mesh and the ssh destinations that
# reach them. `fleet plan` says what a rollout would do and reaches nothing; `fleet status`
# asks every machine what it runs; `fleet rollout` installs the release with the published
# installer and verifies the launcher answers at the version, and a second rollout changes
# nothing. Unpinned, the version is the latest stable release the repository records, and a
# repository that records none has nothing to roll out. A machine no destination reaches is reported, and the others go on.
#
# The machines are directories: `ssh` and `curl` are stubs on PATH, so the destination
# lab@10.0.0.1 is a shell whose home is $S/hosts/lab@10.0.0.1, a destination with no
# directory is unreachable, and the installer the stub serves writes a launcher that
# answers with the version it was asked for. The programs that run there are the real
# ones; only the network is not.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq"
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; MAJORDOMUS_LOG=error; export MAJORDOMUS_SHARE MAJORDOMUS_LOG
S="$(mktemp -d "${TMPDIR:-/tmp}/mj1003.XXXXXX")"; trap 'rm -rf "$S"' EXIT
# a release the fixture names, since a fresh repository records none
VERSION=0.42.0

mkdir -p "$S/bin" "$S/hosts/lab@10.0.0.1" "$S/state"
cat > "$S/bin/ssh" <<'STUB'
#!/bin/sh
# ssh [-o opt]... [--] DEST COMMAND: COMMAND runs with DEST's directory as its home
while [ $# -gt 0 ]; do
  case "$1" in -o) shift 2 ;; --) shift; break ;; *) break ;; esac
done
dest="$1"; shift
h="$FLEET_HOSTS/$dest"
[ -d "$h" ] || { echo "ssh: connect to host $dest port 22: No route to host" >&2; exit 255; }
HOME="$h" exec sh -c "$*"
STUB
cat > "$S/bin/curl" <<'STUB'
#!/bin/sh
# the installer for the install script's URL; nothing answers anywhere else
for a in "$@"; do url="$a"; done
case "$url" in
  */install.sh)
    cat <<'INSTALLER'
v=""
while [ $# -gt 0 ]; do case "$1" in --version) v="${2#v}"; shift 2 ;; *) shift ;; esac; done
mkdir -p "$HOME/.local/bin"
printf '#!/bin/sh\necho "majordomus %s"\n' "$v" > "$HOME/.local/bin/majordomus"
chmod +x "$HOME/.local/bin/majordomus"
echo "installed $v"
INSTALLER
    ;;
  *) exit 7 ;;
esac
STUB
chmod +x "$S/bin/ssh" "$S/bin/curl"
PATH="$S/bin:$PATH"; FLEET_HOSTS="$S/hosts"; XDG_STATE_HOME="$S/state"
export PATH FLEET_HOSTS XDG_STATE_HOME

cd "$T"
git config user.email t@example.com; git config user.name t
"$MJ" init >/dev/null 2>&1; "$MJ" update >/dev/null 2>&1
mkdir -p .ai/repo/fleet
declare_fleet() {
  cat > .ai/repo/fleet/lab.yaml <<EOF
schema: fleet/v1
kind: fleet-declaration
id: lab
machines:
  - id: near
    node: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa01
    ssh: [$1, lab@10.0.0.1]
  - id: gone
    node: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa02
    ssh: [lab@10.0.0.2]
EOF
  git add -A >/dev/null; git commit -qm fleet >/dev/null
}
declare_fleet lab@10.0.0.9
fail() { echo "    $1"; [ -z "${2:-}" ] || head -c 1500 "$2"; echo; exit 1; }

# ---------------------------------------------------------------- 1. plan reaches nothing
"$RB" fleet plan --version "$VERSION" --format json > "$S/plan.json" 2>"$S/err" || fail "fleet plan failed:" "$S/err"
jq -e --arg v "$VERSION" '.version == $v and (.machines | length) == 2' "$S/plan.json" >/dev/null \
  || fail "the plan does not name the version and both machines:" "$S/plan.json"
jq -e '.machines[0].steps == ["claims","reach","probe","install","verify","servers"]
       and .machines[0].destinations == ["lab@10.0.0.9","lab@10.0.0.1"] and .machines[0].local == false' \
  "$S/plan.json" >/dev/null || fail "the plan's steps or destinations are not the declared ones:" "$S/plan.json"
[ ! -e "$S/hosts/lab@10.0.0.1/.local" ] || fail "fleet plan touched a machine"

# ---------------------------------------------------------------- 2. status asks, and says who is gone
expect_exit 10 "$RB" fleet status
"$RB" fleet status --format json > "$S/status.json" 2>/dev/null || true
jq -e '.machines[0].reached_by == "lab@10.0.0.1" and .machines[0].installed == null
       and .machines[0].platform != null' "$S/status.json" >/dev/null \
  || fail "status did not reach near by its second destination:" "$S/status.json"
jq -e '.machines[1].reached_by == null and (.machines[1].error | test("lab@10.0.0.2: .*No route"))' \
  "$S/status.json" >/dev/null || fail "an unreachable machine is not reported with its reason:" "$S/status.json"

# ---------------------------------------------------------------- 3. a rollout installs and verifies
"$RB" fleet rollout --machine near --version "$VERSION" --format json > "$S/r1.json" 2>"$S/err" \
  || fail "rolling out to near did not converge:" "$S/r1.json"
jq -e --arg v "$VERSION" '.verdict == "converged" and .machines[0].verdict == "converged"
       and .machines[0].after == $v
       and ([.machines[0].steps[] | select(.step == "install")][0].status == "changed")' \
  "$S/r1.json" >/dev/null || fail "the rollout did not install and verify the version:" "$S/r1.json"
[ "$("$S/hosts/lab@10.0.0.1/.local/bin/majordomus" --version)" = "majordomus $VERSION" ] \
  || fail "the launcher on near does not answer at $VERSION"

# ---------------------------------------------------------------- 4. a second one changes nothing
"$RB" fleet rollout --machine near --version "$VERSION" --format json > "$S/r2.json" 2>/dev/null \
  || fail "the second rollout did not converge:" "$S/r2.json"
jq -e '[.machines[0].steps[] | select(.step == "install")][0].status == "skipped"' "$S/r2.json" >/dev/null \
  || fail "a second rollout installed again:" "$S/r2.json"

# ---------------------------------------------------------------- 5. one machine gone, the fleet is partial
expect_exit 10 "$RB" fleet rollout --version "$VERSION"
"$RB" fleet rollout --version "$VERSION" --format json > "$S/r3.json" 2>/dev/null || true
jq -e '.verdict == "partial" and .machines[0].verdict == "converged" and .machines[1].verdict == "unreachable"' \
  "$S/r3.json" >/dev/null || fail "one unreachable machine did not leave the fleet partial:" "$S/r3.json"
expect_exit 10 "$RB" fleet rollout --machine nowhere --version "$VERSION"

# ---------------------------------------------------------------- 5b. no release recorded, nothing to install
# unpinned, a rollout installs the latest stable release the repository records; this one
# records none, so it says so and reaches nothing
expect_exit 12 "$RB" fleet rollout --machine near
expect_grep "records no stable release"

# ---------------------------------------------------------------- 6. the rollout is a terminal's alone
"$RB" capabilities describe fleet.rollout --format json > "$S/cap.json" 2>"$S/err" || fail "no fleet.rollout:" "$S/err"
jq -e '.. | objects | select(has("effect")) | .effect == "remote_mutation"' "$S/cap.json" >/dev/null \
  || fail "fleet.rollout is not classified as changing other machines:" "$S/cap.json"

# ---------------------------------------------------------------- 7. a public address is refused
declare_fleet lab@8.8.8.8
expect_exit 10 "$RB" fleet plan --version "$VERSION"
expect_grep "public address"
echo "    ok"
