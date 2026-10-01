# majordomus-covers: none
# A case can name what it covers, whatever kind of thing that is.
#
# `# majordomus-covers:` could name exactly one kind of thing: a public command of the shell
# tool. `lib/commands.sh` refused anything else — "declares coverage of it, but it is not a
# public command" — and the public list is `start check finish … bench archive`. `release` and
# `serve` are not in it; those are the Rust executable's.
#
# So every case whose subject is a gate, a script, a workflow, a projection or a capability of
# the executable had **no word for its own subject**, and `none` was the only legal value. On
# 2026-09-15 that was 74 of 215 cases. The doctrine line reads "command coverage — every public
# command is exercised and refuted", which is true, and which a reader takes for "the tests are
# linked to what they test".
#
# What is asserted, by running the two checkers rather than by re-implementing them: the doctor
# validator (mj_validate_command_coverage) and case 31 both accept a real gate, script, workflow
# and capability; both refuse a missing one of each, a name that is a pattern rather than a
# name, a script path outside scripts/ and an unknown prefix; and — the section that matters
# most — a prefixed name cannot be used to escape the older obligation that every public
# command carries a bare-named case.
#
# The suite is a fixture: a root inside $T with the real command registry, gate model, registry
# projection and a workflow copied in, and a test/cases/ written here. The case declares none
# itself: what it runs is the doctor validator and case 31, and neither is a gate, a script
# under scripts/ or a capability.
. "$ROOT/test/lib.sh"

GATES="$ROOT/.ai/repo/ci/gates.yaml"
[ -f "$GATES" ] || { echo "    no gate model"; exit 1; }

# ---- the fixture root: the models the checkers read, and a suite of our own
FX="$T/fx"
mkdir -p "$FX/bin" "$FX/share" "$FX/.ai/repo/ci" "$FX/docs/generated" "$FX/.github/workflows" \
         "$FX/scripts/ci" "$FX/test/cases"
cp "$ROOT/share/commands.yaml" "$FX/share/"
cp "$GATES" "$FX/.ai/repo/ci/"
cp "$ROOT/docs/generated/registry.json" "$FX/docs/generated/"
cp "$ROOT/.github/workflows/pages.yml" "$FX/.github/workflows/"
cp "$ROOT/test/lib.sh" "$FX/test/"
ln -s "$ROOT/lib" "$FX/lib"
printf '#!/bin/sh\nexit 0\n' > "$FX/scripts/ci/real"; chmod +x "$FX/scripts/ci/real"
printf '#!/bin/sh\nexit 0\n' > "$FX/outside";        chmod +x "$FX/outside"

real_gate="$(grep -m1 -E '^  - id: [a-z][a-z0-9-]*$' "$GATES" | sed 's/.*id: //')"
[ -n "$real_gate" ] || { echo "    the gate model declares no id to test against"; exit 1; }
grep -Fq '"id": "plan.transition"' "$FX/docs/generated/registry.json" \
  || { echo "    the registry projection carries no plan.transition to test against"; exit 1; }

# the public surface, read the way the validator reads it
public="$(MJ_BIN_DIR="$FX/bin" MJ_LIB_DIR="$ROOT/lib" TMPDIR="$T" bash -c '
  . "$MJ_LIB_DIR/common.sh"; . "$MJ_LIB_DIR/commands.sh"
  mj_cmdreg_load && mj_cmdreg_public' | tr '\n' ' ')"
[ -n "$(printf '%s' "$public" | tr -d ' ')" ] || { echo "    the registry declares no public command"; exit 1; }

# A base suite that satisfies every older obligation, so a verdict can only come from the
# probe: every public command covered and refuted by bare name, both lifecycle ends, and enough
# declaring cases for case 31 to believe it is reading headers.
base_suite() {  # <public commands to cover>
  rm -f "$FX"/test/cases/*.sh
  printf '# majordomus-covers: %s\n# majordomus-negative: %s\n' "$1" "$1" > "$FX/test/cases/a_all.sh"
  # case 31 reads a lifecycle line only from a case that covers something
  printf '# majordomus-covers: %s\n# majordomus-lifecycle: accepted\n' "$1" > "$FX/test/cases/b_acc.sh"
  printf '# majordomus-covers: %s\n# majordomus-lifecycle: refused\n' "$1" > "$FX/test/cases/c_ref.sh"
  local i
  for i in 01 02 03 04 05 06 07 08 09 10 11 12 13 14 15; do
    printf '# majordomus-covers: none\n' > "$FX/test/cases/f_$i.sh"
  done
}

# the doctor validator, run for real against the fixture; one line per verdict
doctor_says() {
  MJ_BIN_DIR="$FX/bin" MJ_LIB_DIR="$ROOT/lib" TMPDIR="$T" bash -c '
    . "$MJ_LIB_DIR/common.sh"; . "$MJ_LIB_DIR/commands.sh"
    mj_doctrine_fail() { printf "FAIL %s :: %s\n" "$2" "$3"; }
    mj_doctrine_ok()   { printf "OK %s\n" "$2"; }
    mj_doctrine_skip() { printf "SKIP %s :: %s\n" "$2" "$3"; }
    mj_validate_command_coverage'
}

# case 31, run for real against the fixture: its own ROOT, and a $T of its own beside it
case31_rc() {
  local d="$T/t31" c31="$ROOT/test/cases/31_command_coverage.sh" rc=0
  rm -rf "$d"; mkdir -p "$d"
  ( cd "$d" && ROOT="$FX" T="$d" bash -eu "$c31" ) \
    > "$T/c31.out" 2>&1 || rc=$?
  printf '%s\n' "$rc"
}

# ---- 1. with no probe the fixture is clean in both checkers, so a later verdict is the probe's
base_suite "$public"
doctor_says > "$T/d.out"
grep -q '^OK coverage$' "$T/d.out" && ! grep -q '^FAIL' "$T/d.out" \
  || { echo "    the base fixture is not clean under the doctor validator:"; sed 's/^/      /' "$T/d.out"; exit 1; }
[ "$(case31_rc)" = 0 ] \
  || { echo "    the base fixture is not clean under case 31:"; sed 's/^/      /' "$T/c31.out"; exit 1; }
echo "    the base fixture passes both checkers"

probe() {  # <declared name>: adds one case declaring it; leaves both verdicts in files
  base_suite "$public"
  printf '# majordomus-covers: %s\n' "$1" > "$FX/test/cases/z_probe.sh"
  doctor_says > "$T/d.out"
  case31_rc > "$T/c31.rc"
}

# ---- 2. a real thing of each kind resolves in both
for name in "gate:$real_gate" script:scripts/ci/real workflow:pages.yml capability:plan.transition; do
  probe "$name"
  if grep -q '^FAIL' "$T/d.out"; then
    echo "    the doctor validator refuses '$name', which exists:"; sed 's/^/      /' "$T/d.out"; exit 1
  fi
  grep -q '^OK coverage$' "$T/d.out" || { echo "    the doctor validator did not pass '$name'"; exit 1; }
  [ "$(cat "$T/c31.rc")" = 0 ] \
    || { echo "    case 31 refuses '$name', which exists:"; sed 's/^/      /' "$T/c31.out"; exit 1; }
done
echo "    a real gate, script, workflow and capability resolve in the doctor validator and in case 31"

# ---- 3. what does not name a real thing is refused by both, naming the entry
# `gate:.*` and `capability:plan.transitio.` are here because a name built into a regular
# expression would resolve to an entry it does not name (every gate; plan.transition).
for name in gate:no-such-gate 'gate:.*' script:scripts/nope script:outside \
            script:scripts/../outside workflow:no-such.yml workflow:../workflows/pages.yml \
            capability:nope.nothing capability:plan.transitio. bogus:x; do
  probe "$name"
  grep -Fq "FAIL $name ::" "$T/d.out" \
    || { echo "    the doctor validator does not refuse '$name':"; sed 's/^/      /' "$T/d.out"; exit 1; }
  if grep -q '^OK coverage$' "$T/d.out"; then
    echo "    the doctor validator reports OK beside '$name'"; exit 1
  fi
  [ "$(cat "$T/c31.rc")" = 1 ] \
    || { echo "    case 31 does not refuse '$name' (rc $(cat "$T/c31.rc")):"; sed 's/^/      /' "$T/c31.out"; exit 1; }
  grep -Fq -- "${name#*:}" "$T/c31.out" \
    || { echo "    case 31 refuses '$name' without naming it:"; sed 's/^/      /' "$T/c31.out"; exit 1; }
done
echo "    a missing entry of each kind, a pattern, a path outside scripts/ and an unknown kind are refused"

# ---- 4. THE MUTATION: the second vocabulary cannot satisfy the first
# The obligation that every public command carries a behavioural and a negative case is the
# older and stricter one. Drop one command from the base suite, then make a prefixed name that
# spells it resolve: a gate with the command's own id. If a prefixed name counted as command
# coverage the dropped command would still look covered, and the gate would stay green.
drop="$(printf '%s\n' $public | head -1)"
rest="$(printf '%s\n' $public | sed 1d | tr '\n' ' ')"
base_suite "$rest"
printf '  - id: %s\n' "$drop" >> "$FX/.ai/repo/ci/gates.yaml"
printf '# majordomus-covers: gate:%s\n# majordomus-negative: gate:%s\n' "$drop" "$drop" \
  > "$FX/test/cases/z_probe.sh"
doctor_says > "$T/d.out"
grep -Fq "FAIL $drop :: no test case declares behaviour coverage of it" "$T/d.out" \
  || { echo "    '$drop' is left uncovered, a case names gate:$drop, and the doctor validator does"
       echo "    not say '$drop' lacks coverage: a prefixed name is counting as command coverage"
       sed 's/^/      /' "$T/d.out"; exit 1; }
rc31="$(case31_rc)"
[ "$rc31" = 1 ] && grep -Fq "$drop: no case declares behaviour coverage" "$T/c31.out" \
  || { echo "    case 31 lets gate:$drop stand in for the command '$drop':"; sed 's/^/      /' "$T/c31.out"; exit 1; }
echo "    a prefixed name does not count as command coverage; the older obligation is untouched"

# ---- 5. and it is used, not merely available
# A vocabulary nobody speaks is a vocabulary nobody can be held to. This case declares none,
# and is excluded anyway, so it can never count itself.
used="$(for f in "$ROOT"/test/cases/*.sh; do
          [ "$(basename "$f")" = 617_a_case_can_name_what_it_covers.sh ] && continue
          sed -n '1s/^# majordomus-covers: *//p' "$f"
        done | tr ' ' '\n' | grep -cE '^(gate|script|workflow|capability):' || true)"
[ "${used:-0}" -ge 1 ] \
  || { echo "    no other case uses the new vocabulary, so nothing would notice if it stopped working"; exit 1; }
echo "    $used declaration(s) in other cases name a gate, a script, a workflow or a capability"

echo "    a case can name what it covers, and naming the wrong thing is still refused"
