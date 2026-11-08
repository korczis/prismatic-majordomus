# majordomus-covers: none
# The site-data fixture runs the executable the harness resolved, and names its absence (I4563).
#
# Case 10 builds a repository of one commit and runs its scripts/generate-site-data, which reads
# `plan delivery --at` through the fixture's own bin/majordomus. The fixture has no build of its
# own and no installed release can be of its generation, so the read exited 12 whenever the
# caller had not exported MAJORDOMUS_BIN: the case passed on the pre-push and CI paths and failed
# run by hand (2026-11-08, master 67e9a2f04a7). Case 10 now resolves its executable with
# rust_bin and runs the generator through fixture_tool_env (test/lib.sh).
#
# What is proved, with the generator's own reading function (mj_tool_read, lifted out of
# scripts/generate-site-data) behind the fixture's real launcher and lib/rust_bin.sh, against a
# stub executable that records how it was called:
#   1  a valid executable answers: MAJORDOMUS_BIN with its commit, an installed release of the
#      fixture's generation, which the launcher says it answers through, or the installer's
#      shim rust_bin may answer with, whose compiled executable fixture_tool_env hands over
#   2  a missing executable is 12, the launcher's own line naming `just build` and the
#      generator's naming the read; a MAJORDOMUS_BIN that names nothing is 12, named as gone
#   3  a release of a foreign generation is 12: its commit is not in this repository
#   4  a release whose build commit is unknown is 12: set MAJORDOMUS_BIN_COMMIT
#   5  a caller's foreign variables: through fixture_tool_env the executable reads the fixture's
#      own share and sees none of the caller's MJ_* or MAJORDOMUS_*; without it a foreign
#      MAJORDOMUS_BIN is refused by name, in the git hook's words a foreign MAJORDOMUS_HOOK chose
# Nothing here masks a failed read: mj_tool_read still exits 12 (`project.a-verdict-states-its-subject`).
. "$ROOT/test/lib.sh"
command -v jq >/dev/null || skip "jq absent"

F="$PWD/fixture"
mkdir -p "$F"
cp -R "$ROOT/bin" "$ROOT/lib" "$ROOT/share" "$F/"
git -C "$F" init -q
git -C "$F" add -A >/dev/null
git -C "$F" -c user.email=case@test -c user.name=case commit -qm fixture
H="$(git -C "$F" rev-parse HEAD)"
FOREIGN="$(git -C "$ROOT" rev-parse HEAD)"   # this checkout's HEAD: a real commit, and not one of the fixture's
git -C "$F" rev-parse -q --verify "$FOREIGN^{commit}" >/dev/null && { echo "    the fixture already holds $FOREIGN"; exit 1; }

# The generator's reading function, lifted as it is and run from a driver with the fixture as ROOT.
FN="$PWD/mj_tool_read.sh"
sed -n '/^mj_tool_read() {/,/^}/p' "$ROOT/scripts/generate-site-data" > "$FN"
grep -q '^mj_tool_read() {' "$FN" || { echo "    mj_tool_read could not be lifted out of scripts/generate-site-data"; exit 1; }
grep -q 'exit 12' "$FN" || { echo "    mj_tool_read no longer exits 12 for a read it could not make"; exit 1; }
DRIVER="$PWD/read"
# shellcheck disable=SC2016 # $1 and $@ are the driver's own, written as they are
printf '#!/usr/bin/env bash\nset -eu\nROOT="$1"; shift\n. "%s"\nmj_tool_read "$@"\n' "$FN" > "$DRIVER"; chmod +x "$DRIVER"

# The stub executable: answers --version and one JSON document, and records its argv and the
# MAJORDOMUS_* and MJ_* variables it was handed.
LOG="$PWD/stub.log"
stub_exe() { # <path>
  mkdir -p "$(dirname "$1")"
  printf '#!/usr/bin/env bash\n{ printf "argv %%s\\n" "$*"; env | grep -E "^(MAJORDOMUS|MJ)_" | LC_ALL=C sort; } > "%s"\ncase " $* " in *" --version "*) echo "majordomus 1.0.0"; exit 0 ;; esac\necho "{\\"schema\\":1,\\"issues\\":[]}"\n' "$LOG" > "$1"
  chmod +x "$1"
}
STUB="$PWD/exe/majordomus"; stub_exe "$STUB"
release() { # <dir> [<commit>]  an installed release: the stub, and the stamp scripts/release-package writes
  rm -rf "$1"; stub_exe "$1/bin/majordomus"
  [ -z "${2:-}" ] || printf '{"version": "1.0.0", "commit": "%s"}\n' "$2" > "$1/RELEASE.json"
}

# One read of the issues' delivery at the fixture's HEAD, in an environment of exactly the
# assignments given: the caller's MAJORDOMUS_*, MJ_* and cargo target variables are dropped.
read_with() { # <VAR=value>...   prints the exit code; the output is in read.out
  rm -f "$LOG" "$PWD/read.json" "$PWD/read.json.err"
  ( for v in $(compgen -e); do
      case "$v" in MAJORDOMUS_*|MJ_*|CARGO_TARGET_DIR|CARGO_BUILD_TARGET_DIR) unset "$v" ;; esac
    done
    for a in "$@"; do export "${a?}"; done
    "$DRIVER" "$F" "$PWD/read.json" "the issues' delivery" --json plan delivery --at "$H"
  ) > read.out 2>&1 && echo 0 || echo $?
}
NONE="MAJORDOMUS_INSTALLED_BIN=$PWD/no-release/majordomus"   # no installed release at all
said() { grep -qF -- "$1" read.out || { echo "    $2 did not say: $1"; cat read.out; exit 1; }; }
exit_is() { [ "$1" = "$2" ] || { echo "    $3: exit $1, expected $2"; cat read.out; exit 1; }; }
read_failed_said() {
  said "generate-site-data: the issues' delivery could not be read: 'majordomus --json plan delivery --at $H' exited 12" "$1"
  said "the site is not generated from a subject this run could not reach" "$1"
}

# ---------------------------------------------------------------- 1  a valid executable answers
exit_is "$(read_with "MAJORDOMUS_BIN=$STUB" "MAJORDOMUS_BIN_COMMIT=$H" "$NONE")" 0 "a named executable"
jq -e '.schema == 1' "$PWD/read.json" >/dev/null || { echo "    the read did not write the executable's answer"; exit 1; }
grep -qxF "argv plan delivery --repo $F --format json --at $H" "$LOG" || { echo "    the executable was not asked for the fixture's delivery:"; cat "$LOG"; exit 1; }
grep -qxF "MAJORDOMUS_SHARE=$F/share" "$LOG" || { echo "    the executable was not handed the fixture's share:"; cat "$LOG"; exit 1; }
release "$PWD/rel-own" "$H"
exit_is "$(read_with "MAJORDOMUS_INSTALLED_BIN=$PWD/rel-own/bin/majordomus")" 0 "an installed release of the fixture's generation"
# a read that succeeds keeps the launcher's words beside its answer (mj_tool_read's <dest>.err)
grep -qF "answering through the installed release $PWD/rel-own/bin/majordomus, built from ${H:0:11}" "$PWD/read.json.err" \
  || { echo "    the launcher did not say it answered through the release of the fixture's generation:"; cat "$PWD/read.json.err"; exit 1; }
# what rust_bin answers when the installed release is of the tree's generation is the installer's
# shim, which the fixture's launcher would refuse as a launcher: fixture_tool_env hands over the
# compiled executable the shim's version carries, and its commit from the version's stamp
V="$PWD/installed/versions/1.0.0"
release "$V" "$H"; stub_exe "$V/libexec/majordomus-cli"
printf '#!/bin/sh\n# Generated by the Majordomus installer. Reinstalling rewrites it.\nexec "%s" "$@"\n' "$V/bin/majordomus" > "$PWD/installed/majordomus"
chmod +x "$PWD/installed/majordomus"
rm -f "$LOG"; rc=0
fixture_tool_env "$PWD/installed/majordomus" "$DRIVER" "$F" "$PWD/read.json" "the issues' delivery" --json plan delivery --at "$H" > read.out 2>&1 || rc=$?
exit_is "$rc" 0 "the installer's shim, through fixture_tool_env"
grep -qxF "MAJORDOMUS_BIN=$V/libexec/majordomus-cli" "$LOG" || { echo "    fixture_tool_env did not hand over the shim's compiled executable:"; cat "$LOG"; exit 1; }
grep -qxF "MAJORDOMUS_BIN_COMMIT=$H" "$LOG" || { echo "    fixture_tool_env did not read the shim's commit from its version's stamp:"; cat "$LOG"; exit 1; }
echo "    ok   a_valid_executable_answers_the_fixture"

# ---------------------------------------------------------------- 2  a missing executable is named
exit_is "$(read_with "$NONE")" 12 "no executable and no release"
said "majordomus-cli: $F/apps/majordomus-cli/target/debug/majordomus is missing, and no ordinary command builds it (ADR 0224)" "a missing executable"
said "majordomus-cli:   run: just build" "a missing executable"
read_failed_said "a missing executable"
exit_is "$(read_with "MAJORDOMUS_BIN=$PWD/gone/majordomus" "$NONE")" 12 "a MAJORDOMUS_BIN that names nothing"
said "majordomus-cli: $PWD/gone/majordomus is not an executable" "a MAJORDOMUS_BIN that names nothing"
said "MAJORDOMUS_BIN names it" "a MAJORDOMUS_BIN that names nothing"
read_failed_said "a MAJORDOMUS_BIN that names nothing"
echo "    ok   a_missing_executable_is_named"

# ---------------------------------------------------------------- 3  a foreign generation is refused
release "$PWD/rel-foreign" "$FOREIGN"
exit_is "$(read_with "MAJORDOMUS_INSTALLED_BIN=$PWD/rel-foreign/bin/majordomus")" 12 "a release of a foreign generation"
said "the installed release $PWD/rel-foreign/bin/majordomus is not of this tree's generation: its commit $FOREIGN is not in this repository" "a foreign generation"
read_failed_said "a foreign generation"
# probed for its version (mj_rust_installed), never asked for the reading
! grep -q '^argv plan' "$LOG" 2>/dev/null || { echo "    a release of a foreign generation was asked for the reading:"; cat "$LOG"; exit 1; }
echo "    ok   a_foreign_generation_is_refused"

# ---------------------------------------------------------------- 4  an unknown build commit is refused
release "$PWD/rel-unstamped"
exit_is "$(read_with "MAJORDOMUS_INSTALLED_BIN=$PWD/rel-unstamped/bin/majordomus")" 12 "a release with no build commit"
said "is not of this tree's generation: the commit it was built from is unknown (set MAJORDOMUS_BIN_COMMIT)" "an unknown build commit"
read_failed_said "an unknown build commit"
echo "    ok   an_unknown_build_commit_is_refused"

# ---------------------------------------------------------------- 5  a caller's foreign variables
mkdir -p "$PWD/foreign-share"
foreign=("MAJORDOMUS_BIN=$PWD/gone/majordomus" "MAJORDOMUS_BIN_COMMIT=0123456789abcdef0123456789abcdef01234567" "MAJORDOMUS_SHARE=$PWD/foreign-share"
         "MAJORDOMUS_HOOK=1" "MJ_SITE_SINCE_DEPLOY_RECORD=$PWD/foreign-record" "$NONE")
# without the scrub (the negative check): the foreign MAJORDOMUS_BIN is what the launcher meets
exit_is "$(read_with "${foreign[@]}")" 12 "a read in the caller's foreign environment"
said "majordomus-cli (git hook): $PWD/gone/majordomus is not an executable" "the caller's foreign MAJORDOMUS_BIN"
# through fixture_tool_env, in that same foreign environment: the resolved executable answers
rm -f "$LOG"
rc=0
( for a in "${foreign[@]}"; do export "${a?}"; done
  fixture_tool_env "$STUB" "$DRIVER" "$F" "$PWD/read.json" "the issues' delivery" --json plan delivery --at "$H"
) > read.out 2>&1 || rc=$?
exit_is "$rc" 0 "fixture_tool_env in a foreign environment"
grep -qxF "MAJORDOMUS_BIN=$STUB" "$LOG" || { echo "    fixture_tool_env did not hand over the resolved executable:"; cat "$LOG"; exit 1; }
# a stub with no stamp was built, as far as anyone can know, from this checkout's HEAD
grep -qxF "MAJORDOMUS_BIN_COMMIT=$FOREIGN" "$LOG" || { echo "    fixture_tool_env did not hand over the executable's commit:"; cat "$LOG"; exit 1; }
grep -qxF "MAJORDOMUS_SHARE=$F/share" "$LOG" || { echo "    the caller's MAJORDOMUS_SHARE reached the fixture's executable:"; cat "$LOG"; exit 1; }
# the shell tool sets MJ_* of its own (MJ_BIN_DIR, MJ_SHARE_DIR, ...); none of the caller's arrives
! grep -qE '^(MJ_SITE_SINCE_DEPLOY_RECORD|MAJORDOMUS_HOOK|MAJORDOMUS_INSTALLED_BIN)=' "$LOG" || { echo "    a caller's variable reached the fixture's executable:"; cat "$LOG"; exit 1; }
echo "    ok   a_callers_foreign_variables_do_not_reach_the_fixture"
echo "    ok   the_site_data_fixture_names_its_executable"
