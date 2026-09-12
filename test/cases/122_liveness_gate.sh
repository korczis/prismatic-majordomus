# majordomus-covers: none
# scripts/liveness-check reads this repository's shell for the three shapes the liveness
# doctrine forbids: a network call with no bound on how long it may block, a process put
# into the background whose completion nobody records, and a git subcommand that pages
# straight to a terminal. What must hold is that it reports a regression, stays quiet about
# the debt the baseline already records, and — the part that matters most — does not report
# a sentence as a call.
#
# The last one is why this case exists at all. The first draft of the gate flagged the word
# "watch" in a help string and `$(git log ...)` captured into a variable, neither of which
# can block anything. A check that cannot tell a call from prose teaches its readers to
# ignore it, which is worse than no check.
. "$ROOT/test/lib.sh"

GATE="$ROOT/scripts/liveness-check"
[ -x "$GATE" ] || { echo "    scripts/liveness-check is not executable"; exit 1; }

# ---------------------------------------------------------------- the checkout it ships on
# The gate is green here: every finding it makes is one the baseline records.
out="$("$GATE" 2>&1)" || { echo "    the gate is not green on the checkout it ships on:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }
printf '%s\n' "$out" | grep -q '^liveness-check: OK' \
  || { echo "    a green run did not say so: $out"; exit 1; }

# ---------------------------------------------------------------- a regression is refused
# A fixture repository, because the gate reads `git ls-files`: a file that is not tracked is
# not part of the repository's shell and the gate is right to ignore it.
F="$T/fixture"; mkdir -p "$F/scripts" "$F/.ai/repo"
( cd "$F" && git init -q . && git config user.email t@example.com && git config user.name t ) \
  || { echo "    could not build the fixture"; exit 1; }
cp "$GATE" "$F/scripts/liveness-check"
printf '# nothing here yet\n' > "$F/.ai/repo/liveness-baseline.txt"

# each of the three shapes, one file each
printf '#!/usr/bin/env bash\ncurl -s "http://example.invalid/x" > /tmp/x\n' > "$F/scripts/net"
printf '#!/usr/bin/env bash\nsleep 99 &\necho started\necho and forgotten\n' > "$F/scripts/spawn"
printf '#!/usr/bin/env bash\ngit log --oneline\n' > "$F/scripts/pager"
chmod +x "$F/scripts/net" "$F/scripts/spawn" "$F/scripts/pager"
( cd "$F" && git add -A && git commit -qm fixture ) || { echo "    could not commit the fixture"; exit 1; }

out="$( cd "$F" && ./scripts/liveness-check 2>&1 )" && {
  echo "    the gate passed a tree carrying all three shapes"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1
}
for want in unbounded-network unsupervised-spawn pager-blocks; do
  printf '%s\n' "$out" | grep -q "$want" \
    || { echo "    the gate did not report $want:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }
done

# ---------------------------------------------------------------- and a call is not prose
# The same three words, none of them a call: a usage line, a help string, and a capture.
# The gate must stay silent on every one of them.
rm -f "$F/scripts/net" "$F/scripts/spawn" "$F/scripts/pager"
cat > "$F/scripts/prose" <<'INNER'
#!/usr/bin/env bash
# curl is mentioned here in a comment
usage() { echo "install with: curl -fsSL https://example.invalid/i.sh | sh"; }
subject="$(git log -1 --format=%s)"
changed="$(git diff --name-only HEAD)"
echo "run 'watch majordomus doctor' to see it live"
printf 'and %s\n' "$subject$changed"
INNER
chmod +x "$F/scripts/prose"
( cd "$F" && git add -A && git commit -qm prose ) || { echo "    could not commit the prose fixture"; exit 1; }
out="$( cd "$F" && ./scripts/liveness-check 2>&1 )" || {
  echo "    the gate reported prose as a blocking call:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1
}

# ------------------------------------------------- the supervision window counts shell code
# `$!` is captured a few lines after the spawn, so the window is six lines -- but six lines
# of shell code. A heredoc body and the body of a multi-line single-quoted string are text
# handed to another interpreter: `python3 - <<PY` and `node -e '...'` can carry twenty lines
# of payload between a spawn and the `pid=$!` that reaps it. Counting that payload against
# the budget reported supervised spawns as unsupervised, which is how this gate came to
# accuse `test/lib.sh` of debt it did not have.
cat > "$F/scripts/window-heredoc" <<'INNER'
#!/usr/bin/env bash
python3 - "$1" <<'PY' &
import sys
a = 1
b = 2
c = 3
d = 4
e = 5
f = 6
g = 7
PY
worker=$!
wait "$worker"
INNER
cat > "$F/scripts/window-quoted" <<'INNER'
#!/usr/bin/env bash
case "$1" in
  a) sleep 9 &
    ;;
  b) node -e '
      var a = 1;
      var b = 2;
      var c = 3;
      var d = 4;
      var e = 5;
      var f = 6;
    ' >/dev/null 2>&1 &
    ;;
esac
pid=$!
wait "$pid"
INNER
chmod +x "$F/scripts/window-heredoc" "$F/scripts/window-quoted"
( cd "$F" && git add -A && git commit -qm window ) || { echo "    could not commit the window fixture"; exit 1; }
out="$( cd "$F" && ./scripts/liveness-check 2>&1 )" || {
  echo "    a spawn reaped across an interpreter payload was reported as unsupervised:"
  printf '%s\n' "$out" | sed 's/^/      /'; exit 1
}

# ------------------------------------------------- and the window still refuses a real one
# The control for the section above. Skipping a payload must cost the gate nothing: a spawn
# that nothing reaps is still a finding when the payload is there, and a literal quote in
# double quotes -- `tr -d "'"` -- must not desynchronise the scanner's idea of what is code.
# Without this, "skip the payload" could degrade into "skip everything" and stay green.
cat > "$F/scripts/window-control" <<'INNER'
#!/usr/bin/env bash
python3 - "$1" <<'PY' &
import sys
a = 1
b = 2
c = 3
d = 4
PY
echo "nothing reaps the process above"
INNER
cat > "$F/scripts/quote-control" <<'INNER'
#!/usr/bin/env bash
value="$(printf 'a\n' | tr -d "'")"
sleep 9 &
echo "$value"
echo one
echo two
echo three
echo four
echo five
INNER
# and the window is still six lines wide. Measuring it in shell code rather than physical
# lines widens what it reaches; it must not remove the bound. A `$!` eight code lines from
# the spawn is outside the window and stays a finding -- without this, "count code lines"
# could quietly become "scan to end of file" and every fixture above would still pass.
cat > "$F/scripts/window-distance" <<'INNER'
#!/usr/bin/env bash
sleep 9 &
echo one
echo two
echo three
echo four
echo five
echo six
echo seven
pid=$!
wait "$pid"
INNER
chmod +x "$F/scripts/window-control" "$F/scripts/quote-control" "$F/scripts/window-distance"
( cd "$F" && git add -A && git commit -qm control ) || { echo "    could not commit the control fixture"; exit 1; }
out="$( cd "$F" && ./scripts/liveness-check 2>&1 )" && {
  echo "    an unreaped spawn behind a payload was accepted"; exit 1
}
for want in scripts/window-control scripts/quote-control scripts/window-distance; do
  printf '%s\n' "$out" | grep -q "$want" \
    || { echo "    the gate lost the unsupervised spawn in $want:"; printf '%s\n' "$out" | sed 's/^/      /'; exit 1; }
done

rm -f "$F/scripts/window-heredoc" "$F/scripts/window-quoted" \
      "$F/scripts/window-control" "$F/scripts/quote-control" "$F/scripts/window-distance"
( cd "$F" && git add -A && git commit -qm "drop window fixtures" ) >/dev/null 2>&1

# ---------------------------------------------------------------- the ratchet cannot rot
# A baseline entry that matches nothing is reported, so the list shrinks as files are fixed
# instead of quietly outliving them.
printf 'scripts/gone:unbounded-network\n' >> "$F/.ai/repo/liveness-baseline.txt"
( cd "$F" && git add -A && git commit -qm stale ) >/dev/null 2>&1
out="$( cd "$F" && ./scripts/liveness-check 2>&1 )" && {
  echo "    a baseline entry matching nothing was accepted"; exit 1
}
printf '%s\n' "$out" | grep -q 'match nothing' \
  || { echo "    the gate did not name the stale entry: $out"; exit 1; }

echo "  liveness-check reports the three shapes, ignores prose, and its baseline cannot rot"
