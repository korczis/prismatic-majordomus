# majordomus-covers: none
# The gate of project.the-lease-is-read-once, driven against fixture trees rather than
# against this checkout: a case that asserted this repository's own readers would pass the
# day somebody added one and edited the baseline in the same commit, which is the thing the
# gate exists to make visible. What is asserted here is the gate's behaviour — what it
# refuses, what it deliberately does not refuse, what it says while doing it, and that it
# carries no copy of the two names it looks for.
#
# The refusals are exercised by planting the violation, because a check nobody has watched
# fail is a check nobody knows works: a shell script that `jq`s the lease and a Rust module
# that parses it by its schema string are each written into a clean tree and the gate is
# asked. The non-refusals matter just as much — a gate that flagged the documentation, the
# tests or `.envrc` watching the file would be turned off within a week.
. "$ROOT/test/lib.sh"
GATE="$ROOT/scripts/ci/lease-reader-check"
[ -x "$GATE" ] || { echo "    $GATE is not executable"; exit 1; }

# A tree with the shape the gate reads: the one reader, a document about the lease, a test
# that plants one, an .envrc that watches it, and a shell library that does not touch it.
# Tracked, because `git ls-files` is the denominator CI measures.
fixture() {
  local f="$T/tree$1" path="${2:-state/mcp/server.json}" schema="${3:-majordomus-mcp-lease/v1}"
  rm -rf "$f"
  mkdir -p "$f/apps/majordomus-cli/src" "$f/apps/majordomus-cli/tests" \
           "$f/lib" "$f/scripts" "$f/docs" "$f/test/cases" "$f/.ai/repo"
  { printf 'pub const LEASE_PATH: &str = "%s";\n' "$path"
    printf 'pub const SCHEMA: &str = "%s";\n' "$schema"
    printf 'pub fn read() {}\n'; } > "$f/apps/majordomus-cli/src/lease.rs"
  printf 'watch_file .ai/local/%s\n' "$path" > "$f/.envrc"
  printf '# the lease at .ai/local/%s is a %s document\n' "$path" "$schema" > "$f/docs/MCP.md"
  printf 'printf %s > .ai/local/%s\n' "'{\"schema\":\"$schema\"}'" "$path" > "$f/test/cases/90_mcp.sh"
  printf 'fn t() { let _ = "%s"; }\n' "$schema" > "$f/apps/majordomus-cli/tests/mcp_shared.rs"
  printf 'mj_context_peers() { curl -fsS "$url/api/v1/peers"; }\n' > "$f/lib/context.sh"
  git init -q "$f" >/dev/null 2>&1
  git -C "$f" add -A >/dev/null 2>&1
  printf '%s' "$f"
}
track() { git -C "$1" add -A >/dev/null 2>&1; }

# --- a tree whose only reader is the reader passes, and names it
F="$(fixture 0)"
expect_exit 0 env MJ_ROOT="$F" "$GATE"
expect_grep 'the lease is read by apps/majordomus-cli/src/lease.rs'
expect_exit 0 env MJ_ROOT="$F" "$GATE" --strict
expect_grep 'and by nothing else'

# --- what it does NOT refuse: a gate that cried wolf here is a gate somebody turns off.
# The document, the behavioural case, the crate's own suite and .envrc all name the lease
# in the clean tree above and none of them was reported.
expect_exit 0 env MJ_ROOT="$F" "$GATE"
expect_no_grep 'docs/MCP.md'
expect_no_grep 'test/cases/90_mcp.sh'
expect_no_grep 'mcp_shared.rs'
expect_no_grep '.envrc'

# --- a shell script that pulls the address out of the lease itself is refused, by name
F="$(fixture 1)"
cat > "$F/lib/context.sh" <<'SH'
lease="$MJ_STATE_DIR/mcp/server.json"
url="$(jq -r '.url // empty' "$lease")"
SH
track "$F"
expect_exit 10 env MJ_ROOT="$F" "$GATE"
expect_grep 'FAIL lease-reader lib/context.sh'
expect_grep 'a second parser of the lease'
# and it says what to do instead, rather than only that something is wrong
expect_grep 'serve status'
expect_grep 'project.the-lease-is-read-once'

# --- a Rust module that parses the document by its schema string is refused too: the
# executable's own three readers are what ADR 0035 folded into one, and a fourth is the
# same defect
F="$(fixture 2)"
printf 'fn read() { if v["schema"] == "majordomus-mcp-lease/v1" { } }\n' \
  > "$F/apps/majordomus-cli/src/environment.rs"
track "$F"
expect_exit 10 env MJ_ROOT="$F" "$GATE"
expect_grep 'FAIL lease-reader apps/majordomus-cli/src/environment.rs'

# --- a recipe that greps the file is refused: a `sed` over JSON is the reader this rule
# was written for
F="$(fixture 3)"
printf "url=\$(sed -n 's/.*\"url\":\"\\([^\"]*\\)\".*/\\1/p' .ai/local/state/mcp/server.json)\n" \
  > "$F/scripts/probe"
track "$F"
expect_exit 10 env MJ_ROOT="$F" "$GATE"
expect_grep 'FAIL lease-reader scripts/probe'

# --- the ratchet: a reader recorded in the baseline does not fail, and a second one beside
# it still does, so pre-existing debt blocks nobody and cannot grow
F="$(fixture 4)"
printf 'cat .ai/local/state/mcp/server.json\n' > "$F/scripts/old-reader"
track "$F"
env MJ_ROOT="$F" "$GATE" --write-baseline >/dev/null || { echo "    --write-baseline failed"; exit 1; }
grep -q '^scripts/old-reader$' "$F/.ai/repo/lease-reader-baseline.txt" \
  || { echo "    the baseline does not record the reader it measured"; cat "$F/.ai/repo/lease-reader-baseline.txt"; exit 1; }
expect_exit 0 env MJ_ROOT="$F" "$GATE"
expect_grep '1 known reader'
# the baseline is a ratchet, not an exemption: --strict still refuses it
expect_exit 10 env MJ_ROOT="$F" "$GATE" --strict
# a new reader beside a known one is new debt
printf 'jq .url .ai/local/state/mcp/server.json\n' > "$F/scripts/new-reader"
track "$F"
expect_exit 10 env MJ_ROOT="$F" "$GATE"
expect_grep 'FAIL lease-reader scripts/new-reader'
# and a reader that is gone is reported without failing, so the baseline can be tightened
rm -f "$F/scripts/old-reader" "$F/scripts/new-reader"
track "$F"
expect_exit 0 env MJ_ROOT="$F" "$GATE"
expect_grep 'a reader is gone'

# --- the two names are read out of the reader, never copied into the gate: a tree that
# calls its lease something else is measured by ITS names, and this repository's names mean
# nothing there
F="$(fixture 5 "state/rendezvous.json" "other-lease/v2")"
printf 'jq .url .ai/local/state/rendezvous.json\n' > "$F/scripts/reads-theirs"
printf 'echo .ai/local/state/mcp/server.json majordomus-mcp-lease/v1\n' > "$F/scripts/reads-ours"
track "$F"
expect_exit 10 env MJ_ROOT="$F" "$GATE"
expect_grep 'FAIL lease-reader scripts/reads-theirs'
expect_no_grep 'scripts/reads-ours'

# --- a reader that declares neither name makes the gate refuse to measure. A gate that
# quietly passed because it could not find what to look for is the failure class this
# repository has paid for more than once.
F="$(fixture 6)"
printf 'pub fn read() {}\n' > "$F/apps/majordomus-cli/src/lease.rs"
track "$F"
expect_exit 12 env MJ_ROOT="$F" "$GATE"
expect_grep 'declares no LEASE_PATH and SCHEMA'

# --- a tree with no reader at all is not this repository and is not a failure
F="$(fixture 7)"
rm -f "$F/apps/majordomus-cli/src/lease.rs"
track "$F"
expect_exit 0 env MJ_ROOT="$F" "$GATE"
expect_grep 'nothing to measure'

# --- the gate is not its own finding
# Asserted against this checkout rather than a fixture, and deliberately, because the defect
# only exists here: the script names the lease path, its bare file name and the schema in
# order to search the tree for them, so read by its own subject rule it is a second parser of
# the lease. It reported itself, `always: true` in gates.yaml made that red in every plan on
# master, and an empty baseline could not exempt it (2026-09-10).
#
# What is asserted is only that the gate never names its own source as a finding. It is not
# an assertion about this repository's readers — the header explains why that would be the
# wrong thing to hold — so the exit status is deliberately not part of it: whatever else the
# trunk owes, the answer may not be the auditor.
out="$("$GATE" 2>&1 || true)"
case "$out" in
  *"scripts/ci/lease-reader-check"*)
    echo "    the gate reported itself as a reader of the lease:"
    printf '%s\n' "$out" | sed 's/^/      /'
    exit 1 ;;
esac

echo "    the lease has one reader, and a second one is refused by name in every language it could be written in"
