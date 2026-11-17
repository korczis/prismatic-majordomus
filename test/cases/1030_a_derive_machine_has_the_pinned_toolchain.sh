# majordomus-covers: doctrine doctor
# majordomus-negative: doctrine doctor
# claims: doctrine-registry
# A machine that derives has the pinned Rust toolchain, and doctor says so before the derive.
#
# `scripts/derive` builds the Rust executable. On 2026-10-10 nine `majordomus prs repair
# --apply` calls were refused on one clone with `env: cargo: No such file or directory`, after
# the merge had been made and the lock taken, while the other two machines of the mesh held the
# toolchain. Nothing in git says what a machine has, so the policy declares it
# (`derive-toolchain`, `wired_by: toolchain:rust-toolchain.toml`) and doctor asks the machine.
# This case drives the verifier through the three states that must differ — no cargo, cargo
# without the pinned channel, the pinned channel — against stand-ins for cargo and rustup in a
# private PATH and HOME, so the answer never depends on what the runner has installed.
. "$ROOT/test/lib.sh"
# The fixture is the directory the case starts in, carrying this repository's own policy, so
# the entry under test is the one the repository declares and not a copy kept here.
fixture_repo "$T" docs
cp "$ROOT/rust-toolchain.toml" "$T/"
MJ="$T/bin/majordomus"

grep -q 'name: derive-toolchain' .ai/repo/policy.yaml || {
  echo "    the policy declares no derive-toolchain entry, so doctor cannot see a machine without cargo"; exit 1; }
grep -q 'wired_by: toolchain:rust-toolchain.toml' .ai/repo/policy.yaml || {
  echo "    derive-toolchain is not wired_by toolchain:rust-toolchain.toml"; exit 1; }
chan="$(sed -n 's/^channel[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' rust-toolchain.toml | head -1)"
[ -n "$chan" ] || { echo "    rust-toolchain.toml pins no channel; this case would pass vacuously"; exit 1; }

# The declared hooks: a fresh fixture has none, and their absence is a real doctor failure that
# would mask the entry this case is about (test/cases/18_doctrine_wiring.sh does the same).
mkdir -p .git/hooks
printf '#!/usr/bin/env bash\nmajordomus doctor\n' > .git/hooks/pre-commit
printf '#!/usr/bin/env bash\nmajordomus finish --check\n' > .git/hooks/pre-push
chmod +x .git/hooks/pre-commit .git/hooks/pre-push

# A private world: a HOME with no ~/.cargo, and a PATH of system directories only, so a cargo
# on the runner is invisible and each state below is the one the case made.
STUB="$T/stub"; mkdir -p "$STUB" "$T/home"
doctor_line() { # prints doctor's line for the entry; PATH and HOME are the private ones
  (cd "$T" && HOME="$T/home" PATH="$STUB:/usr/bin:/bin" "$MJ" doctor) > "$T/doctor.out" 2>&1 || true
  grep -E 'wiring +derive-toolchain' "$T/doctor.out" || true
}

# 1. no cargo anywhere: red, naming cargo and the remedy
line="$(doctor_line)"
printf '%s' "$line" | grep -qE '^FAIL +wiring +derive-toolchain' || {
  echo "    a machine with no cargo was not refused: ${line:-<no line>}"; grep -E 'wiring|FAIL|derive-toolchain' "$T/doctor.out" | head -12 | sed 's/^/    | /'; exit 1; }
printf '%s' "$line" | grep -q 'cargo is not installed' || {
  echo "    the refusal does not say cargo is missing: $line"; exit 1; }

# 2. cargo, and rustup that does not list the pinned channel: red, naming the channel
printf '#!/bin/sh\necho "cargo 1.0.0"\n' > "$STUB/cargo"
printf '#!/bin/sh\n[ "$1 $2" = "toolchain list" ] && echo "stable-x86_64-unknown-linux-gnu (default)"; exit 0\n' > "$STUB/rustup"
chmod +x "$STUB/cargo" "$STUB/rustup"
line="$(doctor_line)"
printf '%s' "$line" | grep -qE '^FAIL +wiring +derive-toolchain' || {
  echo "    cargo without the pinned channel was accepted: ${line:-<no line>}"; exit 1; }
printf '%s' "$line" | grep -qF "pinned toolchain $chan is not installed" || {
  echo "    the refusal does not name the pinned channel $chan: $line"; exit 1; }

# 3. the pinned channel is installed: green
printf '#!/bin/sh\n[ "$1 $2" = "toolchain list" ] && echo "%s-x86_64-unknown-linux-gnu (active)"; exit 0\n' "$chan" > "$STUB/rustup"
line="$(doctor_line)"
printf '%s' "$line" | grep -qE '^OK +wiring +derive-toolchain' || {
  echo "    a machine with the pinned toolchain was refused: ${line:-<no line>}"; exit 1; }

# 4. a toolchain file that pins nothing is refused rather than read as "any cargo will do"
printf '[toolchain]\ncomponents = []\n' > rust-toolchain.toml
line="$(doctor_line)"
printf '%s' "$line" | grep -qE '^FAIL +wiring +derive-toolchain' || {
  echo "    a toolchain file with no channel was accepted: ${line:-<no line>}"; exit 1; }
