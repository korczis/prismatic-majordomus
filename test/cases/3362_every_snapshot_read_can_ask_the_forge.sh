# Every step of the Pages workflow that reads a surface into a snapshot can ask the forge
# (I2964). The pages a snapshot holds read GitHub through gh while they are rendered (the
# monitor, the integration queue, the pull requests), and gh in a workflow answers only with a
# token. The UI and Cockpit read steps had none: the published /cockpit/monitor said "Nothing
# happened in the last six hours" beside its own "set the GH_TOKEN environment variable", while
# two merges and three deployments had happened in those hours. A read step that cannot ask
# publishes an absence as a fact.
. "$ROOT/test/lib.sh"
WF="$ROOT/.github/workflows/pages.yml"
expect_file "$WF"

# One line per step of the workflow: its name, whether it reads a snapshot (`web report`),
# and whether its env carries GH_TOKEN. A step starts at "- name:" or "- uses:" under steps.
steps="$(awk '
  function flush() { if (name != "") print name "\t" reads "\t" token }
  /^      - (name|uses):/ { flush(); name = $0; sub(/^      - (name|uses): */, "", name); reads = 0; token = 0; next }
  /^  [A-Za-z0-9_-]+:[[:space:]]*$/ { flush(); name = "" }
  name != "" && /web report/ { reads = 1 }
  name != "" && /^          GH_TOKEN:/ { token = 1 }
  END { flush() }
' "$WF")"

readers="$(printf '%s\n' "$steps" | awk -F '\t' '$2 == 1')"
[ -n "$readers" ] || { echo "    no step of pages.yml reads a snapshot with web report: the parse is wrong"; exit 1; }
# the two surfaces the site publishes are among them
for surface in "the UI, read once" "the Cockpit, read once"; do
  printf '%s\n' "$readers" | grep -q "^$surface" || { echo "    no read step named '$surface'"; exit 1; }
done

missing="$(printf '%s\n' "$readers" | awk -F '\t' '$3 != 1 { print $1 }')"
if [ -n "$missing" ]; then
  echo "    these snapshot read steps cannot ask the forge (no GH_TOKEN in their env):"
  printf '%s\n' "$missing" | sed 's/^/      /'
  exit 1
fi

# The parse itself is held: a read step without the token is found.
mutated="$T/pages.yml"
awk '/^      - name: the Cockpit, read once/ { cut = 1 } cut && /^          GH_TOKEN:/ { cut = 0; next } { print }' "$WF" \
  | awk '!(/^        env:$/ && prev ~ /timeout-minutes: 9/) { print } { prev = $0 }' > "$mutated"
grep -q 'the Cockpit, read once' "$mutated" || { echo "    the mutation lost the step"; exit 1; }
again="$(awk '
  function flush() { if (name != "") print name "\t" reads "\t" token }
  /^      - (name|uses):/ { flush(); name = $0; sub(/^      - (name|uses): */, "", name); reads = 0; token = 0; next }
  /^  [A-Za-z0-9_-]+:[[:space:]]*$/ { flush(); name = "" }
  name != "" && /web report/ { reads = 1 }
  name != "" && /^          GH_TOKEN:/ { token = 1 }
  END { flush() }
' "$mutated" | awk -F '\t' '$2 == 1 && $3 != 1 { print $1 }')"
case "$again" in
  *"the Cockpit, read once"*) ;;
  *) echo "    a Cockpit read step without GH_TOKEN was not found by the check: '$again'"; exit 1 ;;
esac
exit 0
