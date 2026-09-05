# majordomus-covers: doctor
# majordomus-negative: doctor
#
# The two command doctrines, proved the way every doctrine in this repository is proved:
# by breaking the thing they are supposed to notice and requiring doctor to notice it.
#
# A doctrine that is declared, dispatched and never able to fail is the exact shape of
# defect the doctrine layer exists to catch, so it must not be the shape of these two.
. "$ROOT/test/lib.sh"

W="$T/copy"
fixture_repo "$W" AGENTS.md docs site/data/marketing.toml site/data/nav.toml site/content-src test/cases test/lib.sh
MJC="$W/bin/majordomus"
"$MJC" init >/dev/null
"$MJC" update >/dev/null

# Every probe below asserts it took effect before asserting what it caused. A mutation that
# silently stops mutating leaves a case that passes while proving nothing.
took()  { grep -q "$1" "$2" || { echo "    the probe did not take: expected /$1/ in $2"; exit 1; }; }
gone()  { grep -q "$1" "$2" && { echo "    the probe did not take: /$1/ still in $2"; exit 1; }; return 0; }

# ---- baseline: both doctrines pass, and they are actually running ----------------------
expect_exit 10 "$MJC" doctor          # 10: the hooks are not wired in this throwaway repo
expect_grep 'OK   command +surface'
expect_grep 'OK   command +coverage'
# ... and they are declared, so doctor's own wiring verifier has checked their chain
expect_exit 0 "$MJC" doctrine show majordomus.command-surface
expect_grep 'command_surface'
expect_exit 0 "$MJC" doctrine show majordomus.command-coverage
expect_grep 'command_coverage'

# ---- 1. a command the binary dispatches that the registry does not describe -------------
python3 - "$W/share/commands.yaml" <<'PY' 2>/dev/null || { echo "    python3 absent; skipping"; exit 0; }
import sys
p = sys.argv[1]
s = open(p).read()
i = s.index('  - id: search\n')
j = s.index('  - id: ', i + 10)
open(p, 'w').write(s[:i] + s[j:])
PY
gone '^  - id: search$' "$W/share/commands.yaml"
expect_exit 10 "$MJC" doctor
expect_grep 'FAIL command +search — dispatched by bin/majordomus but absent'
cp "$ROOT/share/commands.yaml" "$W/share/commands.yaml"
took '^  - id: search$' "$W/share/commands.yaml"
expect_exit 10 "$MJC" doctor
expect_grep 'OK   command +surface'

# ---- 2. a public command that no case declares a failure mode of -----------------------
sed -i.bak '/^# majordomus-negative: /d' "$W/test/cases/24_prompt_search.sh"
rm -f "$W/test/cases/24_prompt_search.sh.bak"
gone '^# majordomus-negative: ' "$W/test/cases/24_prompt_search.sh"
expect_exit 10 "$MJC" doctor
expect_grep 'FAIL command +search — no test case declares a failure mode'
expect_grep 'FAIL command +prompt — no test case declares a failure mode'
cp "$ROOT/test/cases/24_prompt_search.sh" "$W/test/cases/24_prompt_search.sh"
took '^# majordomus-negative: ' "$W/test/cases/24_prompt_search.sh"
expect_exit 10 "$MJC" doctor
expect_grep 'OK   command +coverage'

# ---- 3. a header naming a command that does not exist is a broken reference -------------
sed -i.bak 's/^# majordomus-covers: prompt search$/# majordomus-covers: prompt search nosuchcommand/' "$W/test/cases/24_prompt_search.sh"
rm -f "$W/test/cases/24_prompt_search.sh.bak"
took 'nosuchcommand' "$W/test/cases/24_prompt_search.sh"
expect_exit 10 "$MJC" doctor
expect_grep 'FAIL command +nosuchcommand — a test case declares coverage of it'
cp "$ROOT/test/cases/24_prompt_search.sh" "$W/test/cases/24_prompt_search.sh"

# ---- 4. the coverage doctrine skips where there is nothing to measure -------------------
# It is a rule about Majordomus's own suite. In an installation that carries no suite it must
# report a skip rather than a pass, because a rule that cannot run has not been satisfied.
rm -rf "$W/test/cases"
expect_exit 10 "$MJC" doctor
expect_grep 'INFO command +coverage — this installation carries no test suite'
expect_no_grep 'OK   command +coverage'
