# majordomus-covers: none
# What a provider's adapter can do is declared once, and the two halves that hold it agree.
#
# A surface that wants to say "Claude Code fires SessionStart, SessionEnd and PreCompact,
# and archives prompts" had two ways to find out and both were wrong. Reading
# `.claude/hooks/` reports an *installation* as a capability: a checkout where nobody ran
# `majordomus capture install` is told the provider has no lifecycle, and a checkout where
# somebody hand-wrote a shim is told it has one the tool cannot drive. Writing the list into
# a page is a second source of truth that goes stale the day an adapter changes, which is the
# class of defect this repository has been bitten by repeatedly.
#
# So `share/providers.yaml` declares it, and the Rust executable reads it there for
# `lifecycle.providers`, the Cockpit and the HTTP API. The adapter table itself — the shims,
# the env var, the provider's own payload keys — stays in `MJ_CAPTURE_LIFECYCLE` in
# lib/capture.sh, because that is the half the shell drives and moving it is its own work.
# Two halves of one fact is exactly the arrangement that drifts, so this case refuses the
# drift: every provider the shell drives a lifecycle for declares the same events in the
# YAML, no other provider claims any, and prompt capture is declared where the shell adapts
# it and nowhere else.
#
# The long-term fix is for lib/capture.sh to read the YAML rather than carry its own table
# (ADR 0041 names the direction: one domain service, everything else an adapter). Until it
# does, this gate is what makes the duplication safe rather than silent.
. "$ROOT/test/lib.sh"

CAP="$ROOT/lib/capture.sh"
YAML="$ROOT/share/providers.yaml"
expect_file "$CAP"
expect_file "$YAML"

# The tables are single-quoted shell assignments, one row per line. Read from the
# assignment to the line that closes the quote, so a table that grows a second row is read
# whole — and read with a state machine rather than a sed range, because a sed range whose
# end pattern also matches a comment line above the next assignment silently swallows the
# prose in between, which is how the first draft of this case reported that lib/capture.sh
# drives an event called `and`.
table() {
  awk -v v="$1" '
    index($0, v "=\047") == 1 {
      line = substr($0, length(v) + 3)
      if (sub(/\047$/, "", line)) { print line; exit }
      print line; on = 1; next
    }
    on { if (sub(/\047$/, "", $0)) { print; exit } print }
  ' "$CAP"
}

lifecycle_rows="$(table MJ_CAPTURE_LIFECYCLE)"
capture_rows="$(table MJ_CAPTURE_ADAPTERS)"
[ -n "$lifecycle_rows" ] && [ -n "$capture_rows" ] || {
  echo "    the adapter tables in lib/capture.sh could not be read."
  echo "    This case reads the table the shell actually drives; if its shape changed, change this reader with it."
  exit 1
}

# Which columns hold the provider's own event names, from the declaration that says so
# rather than from three numbers written here.
cols="$(table MJ_LIFECYCLE_COLUMNS | awk 'NF { printf "%s ", $2 }')"
[ -n "$cols" ] || { echo "    MJ_LIFECYCLE_COLUMNS could not be read"; exit 1; }

shell_providers="$(printf '%s\n' "$lifecycle_rows" | awk 'NF { print $1 }' | LC_ALL=C sort -u)"
[ -n "$shell_providers" ] || { echo "    no provider rows in MJ_CAPTURE_LIFECYCLE"; exit 1; }

# Every provider the YAML declares, with its lifecycle events and its prompt_capture flag.
# Read with awk and exact string comparison: a provider id is data, and a reader that let it
# reach a regular expression would be a reader a provider named `c++` could break.
yaml_lifecycle() {   # yaml_lifecycle PROVIDER
  awk -v want="$1" '
    /^  [a-z][a-z0-9_-]*:$/ { name = substr($1, 1, length($1) - 1); inl = 0; next }
    /^    lifecycle:$/ { inl = (name == want); next }
    inl && /^      - / { sub(/^      - /, ""); print; next }
    inl && !/^      - / { inl = 0 }
  ' "$YAML" | LC_ALL=C sort -u
}
yaml_declares_lifecycle="$(awk '
  /^  [a-z][a-z0-9_-]*:$/ { name = substr($1, 1, length($1) - 1) }
  /^    lifecycle:$/ { print name }
' "$YAML" | LC_ALL=C sort -u)"
yaml_declares_capture="$(awk '
  /^  [a-z][a-z0-9_-]*:$/ { name = substr($1, 1, length($1) - 1) }
  /^    prompt_capture: true$/ { print name }
' "$YAML" | LC_ALL=C sort -u)"

fail=0
for p in $shell_providers; do
  driven="$(printf '%s\n' "$lifecycle_rows" | awk -v p="$p" -v c="$cols" '
    $1 == p { n = split(c, want, " "); for (i = 1; i <= n; i++) if ($want[i] != "") print $want[i] }
  ' | LC_ALL=C sort -u)"
  declared="$(yaml_lifecycle "$p")"
  if [ "$(echo $driven)" != "$(echo $declared)" ]; then
    fail=1
    printf '    %s: lib/capture.sh drives [%s]; share/providers.yaml declares [%s]\n' \
      "$p" "$(echo $driven)" "$(echo $declared)"
  fi
done

for p in $yaml_declares_lifecycle; do
  printf '%s\n' "$shell_providers" | grep -qxF "$p" || {
    fail=1
    printf '    %s declares a lifecycle in share/providers.yaml and lib/capture.sh drives no adapter for it\n' "$p"
  }
done

shell_capture="$(printf '%s\n' "$capture_rows" | awk 'NF { print $1 }' | LC_ALL=C sort -u)"
if [ "$(echo $shell_capture)" != "$(echo $yaml_declares_capture)" ]; then
  fail=1
  printf '    prompt capture: lib/capture.sh adapts [%s]; share/providers.yaml declares [%s]\n' \
    "$(echo $shell_capture)" "$(echo $yaml_declares_capture)"
fi

[ "$fail" = 0 ] || {
  echo '    Change share/providers.yaml and lib/capture.sh in the same commit: one is what the'
  echo '    shell drives, the other is what every reader outside the shell is told.'
  exit 1
}
printf '    %s provider(s) with a lifecycle adapter, declared identically in both halves\n' \
  "$(printf '%s\n' "$shell_providers" | wc -l | tr -d ' ')"
