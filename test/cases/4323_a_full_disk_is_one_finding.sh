# majordomus-covers: doctor
# A full disk is one finding (I4323).
#
# Doctor's checks read scratch files through mktemp. On a full or unwritable scratch volume
# the entry point died before doctor ran (mktemp's own message, exit 1), and where it got
# further each check that read a scratch file failed on its own with its own remedy: the
# projection regions said `majordomus update`, which writes. The pre-commit hook passed that
# list to the adopter as the reason a commit was refused (OSCILLA, 2026-10-28).
#
# Now doctor asks the scratch volume and the git directory first, and a volume that cannot
# take a scratch file is the one FAIL: `disk`, with a remedy that reads. Everything else is
# not judged. A directory the case cannot write stands for a full scratch volume, and a floor
# above the volume's free space stands for a volume just short of full.
# MUTATION, RUN: return early from mj_doctor_disk_short -> the two runs below exit 12 with
# the projection FAILs and their `majordomus update` remedy.
. "$ROOT/test/lib.sh"

"$MJ" init >/dev/null
git add -A >/dev/null; git commit -qm "adopt" >/dev/null

# A FAIL that is not disk, or a remedy that writes, in the last output.
only_disk() {
  local fails
  fails="$(printf '%s\n' "$LAST_OUT" | grep -E '^FAIL' || true)"
  [ -n "$fails" ] || { echo "    no FAIL at all:"; printf '%s\n' "$LAST_OUT"; return 1; }
  if printf '%s\n' "$fails" | grep -vqE '^FAIL disk '; then
    echo "    a FAIL other than disk:"; printf '%s\n' "$fails" | grep -vE '^FAIL disk '; return 1
  fi
  [ "$(printf '%s\n' "$fails" | wc -l | tr -d ' ')" = 1 ] \
    || { echo "    disk is said more than once:"; printf '%s\n' "$fails"; return 1; }
  if printf '%s\n' "$LAST_OUT" | grep -q 'majordomus update'; then
    echo "    a remedy that writes:"; printf '%s\n' "$LAST_OUT" | grep 'majordomus update'; return 1
  fi
}

# --- a scratch volume that cannot be written: one FAIL, disk, and nothing judged after it
#     (a root user writes through any mode, so the directory stands for nothing there)
if [ "$(id -u)" != 0 ]; then
  RO="$T/ro"; mkdir -p "$RO"; chmod 555 "$RO"
  ro_doctor() { TMPDIR="$RO" "$MJ" doctor; }
  expect_exit 10 ro_doctor
  only_disk
  expect_grep "^FAIL disk +$RO — no space left on device"
  expect_grep 'INFO disk +doctor — not judged: disk'

  # ...and the pre-commit hook init tells an adopter to add refuses a commit with that one line
  printf '#!/bin/sh\nMAJORDOMUS_DOCTOR_CONTEXT=commit %s doctor || exit $?\n' "$MJ" > .git/hooks/pre-commit
  chmod +x .git/hooks/pre-commit
  echo note > note.txt; git add note.txt
  ro_commit() { TMPDIR="$RO" git commit -qm "a commit on a full disk"; }
  before="$(git rev-parse HEAD)"
  # git exits 1 for any refusing hook, whatever the hook exited with
  expect_exit 1 ro_commit
  [ "$(git rev-parse HEAD)" = "$before" ] || { echo "    the commit was made on a full disk"; exit 1; }
  only_disk
  rm -f .git/hooks/pre-commit; git reset -q note.txt; rm -f note.txt
  chmod 755 "$RO"
fi

# --- a volume with less free than the floor: the same one finding, once for the volume
floor_doctor() { MAJORDOMUS_DISK_FLOOR_MB=999999999 "$MJ" doctor; }
expect_exit 10 floor_doctor
only_disk
expect_grep '^FAIL disk .* free, under the floor of 999999999 MB\)'

# --- a volume above the floor says nothing about the disk, and judges everything as before
expect_exit 12 "$MJ" doctor
expect_no_grep '^(FAIL|INFO) disk '
expect_grep '^FAIL projection +AGENTS\.md'
