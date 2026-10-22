# majordomus-covers: none
# A case that switches its mesh's discovery off says so for the tailnet as well (I3837). The
# tailnet provider is on by default (I2410), so a fixture that writes `multicast: enabled:
# false` to stay hermetic still asks the real tailnet on a machine whose PATH has the
# Tailscale CLI, and its assertions about the nodes it hears then depend on that machine:
# on macbook-2 on 2026-10-21 a fixture of that shape heard two real nodes. Every case whose
# declaration turns multicast off also turns the tailnet off; this case reads them all, and
# proves on a fixture that one which does not is named.
. "$ROOT/test/lib.sh"

# the cases whose text turns multicast off and does not, next, turn the tailnet off: written
# on one line, in a printf format, or as a here-document, whose
# lines follow each other (494 and 495 write theirs that way)
unquiet() {
  for f in "$1"/*.sh; do
    if grep -q 'multicast:\\n  enabled: false\\n' "$f" \
      && grep 'multicast:\\n  enabled: false\\n' "$f" \
        | grep -vq 'multicast:\\n  enabled: false\\ntailscale:\\n  enabled: false\\n'; then
      basename "$f"; continue
    fi
    awk '
      p2 ~ /^multicast:[[:space:]]*$/ && p1 ~ /^  enabled: false[[:space:]]*$/ && $0 !~ /^tailscale:/ { bad = 1 }
      { p2 = p1; p1 = $0 }
      END { exit !bad }
    ' "$f" && basename "$f"
  done
  return 0
}

# the fixture: one quiet case, one that forgets the tailnet
mkdir -p "$T/cases"
printf '%s\n' "printf 'enabled: true\\nmulticast:\\n  enabled: false\\ntailscale:\\n  enabled: false\\n'" > "$T/cases/1_quiet.sh"
printf '%s\n' "printf 'enabled: true\\nmulticast:\\n  enabled: false\\ntrust:\\n  policy: tofu\\n'" > "$T/cases/2_loud.sh"
# one argument per line, so that this file's own text holds neither form
printf '%s\n' 'cat <<EOF' 'enabled: true' 'multicast:' '  enabled: false' 'tailscale:' '  enabled: false' EOF > "$T/cases/3_quiet_heredoc.sh"
printf '%s\n' 'cat <<EOF' 'enabled: true' 'multicast:' '  enabled: false' 'trust:' '  policy: tofu' EOF > "$T/cases/4_loud_heredoc.sh"
[ "$(unquiet "$T/cases" | tr '\n' ' ')" = "2_loud.sh 4_loud_heredoc.sh " ] \
  || { echo "    the check did not name the case that leaves the tailnet on: $(unquiet "$T/cases")"; exit 1; }

# the repository's own cases
found="$(unquiet "$ROOT/test/cases")"
[ -z "$found" ] || { echo "    these cases turn multicast off but leave the tailnet on:"; printf '      %s\n' $found; exit 1; }
echo "    every case that turns multicast off turns the tailnet off too, and one that does not is named"
