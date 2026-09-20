# majordomus-covers: none
# The gate of "a command this project documents is one this project runs", driven against
# fixture trees as well as against this checkout.
#
# What the gate decides, and what it deliberately leaves alone. The documents publish 345
# fenced blocks and most of them are not instructions: a ```mermaid fence is a diagram, a
# ```console fence is recorded output, ```yaml and ```json are data. Only ```bash, ```sh and
# ```just say "run this", so only their lines are asked for a proof — and inside them, only
# the lines that call this project's own tools, because `git` and `curl` are not ours to keep
# working, and a line carrying a `<placeholder>` is a shape nobody can run as written.
#
# A proof is a join, not a new test: a use-case scenario that recorded itself running the
# command, a command-line example that `cli_examples.rs` runs, or an explicit
# `<!-- majordomus:unrun <reason> -->` on the fence. The reason is required, because a bare
# exemption is the silent promise this gate exists to end.
#
# Both directions are exercised. The refusals are planted, because a check nobody has watched
# fail is a check nobody knows works; the non-refusals matter as much, since a gate that
# flagged a diagram or a placeholder would be turned off within a week.
. "$ROOT/test/lib.sh"
GATE="$ROOT/scripts/ci/doc-command-check"
[ -x "$GATE" ] || { echo "    $GATE is not executable"; exit 1; }

# A tree of the shape the gate reads: documents, the two recorded corpora it joins against,
# and a baseline. <1> names the fixture, <2> is the body of docs/GUIDE.md.
fixture() { # <n> <document body>
  local f="$T/tree$1"
  rm -rf "$f"; mkdir -p "$f/docs" "$f/site/data/generated" "$f/docs/generated" "$f/.ai/repo"
  printf '%s\n' "$2" > "$f/docs/GUIDE.md"
  cat > "$f/site/data/generated/catalogue.json" <<'JSON'
{ "use_cases": [ { "id": "start-work", "evidence": { "steps": [
  { "command": "majordomus doctor", "exit": 0, "output": "ok" } ] } } ] }
JSON
  cat > "$f/docs/generated/cli.json" <<'JSON'
{ "cli": { "commands": [ { "examples": [ { "id": "serve-status", "argv": ["serve", "status"] } ] } ] } }
JSON
  printf '# baseline\n' > "$f/.ai/repo/doc-command-baseline.txt"
  printf '%s' "$f"
}

# 1. this checkout passes, and says what it measured rather than saying nothing
expect_exit 0 "$GATE"
expect_grep 'documented command\(s\), each executed, marked, or known debt'

# 2. a documented command nothing runs is refused, and the finding names the file and the
#    command rather than the count alone
t="$(fixture A '```bash
majordomus worktree migrate
```')"
MJ_ROOT="$t" expect_exit 10 "$GATE"
expect_grep 'docs/GUIDE.md: a documented command nothing runs'
expect_grep 'command majordomus worktree migrate'
expect_grep 'majordomus:unrun'

# 3. the same command, recorded by a use-case scenario, is proof: the join is what closes it
t="$(fixture B '```bash
majordomus doctor
```')"
MJ_ROOT="$t" expect_exit 0 "$GATE"

# 4. and so is an executed command-line example, read from the command line'"'"'s own document
t="$(fixture C '```bash
majordomus serve status
```')"
MJ_ROOT="$t" expect_exit 0 "$GATE"

# 5. a marked fence passes — but the marker must carry its reason. A bare marker is the
#    silent exemption this gate exists to refuse, so it is refused by name.
t="$(fixture D '<!-- majordomus:unrun needs a published domain, which no test tree has -->
```bash
scripts/site-deploy
```')"
MJ_ROOT="$t" expect_exit 0 "$GATE"

t="$(fixture E '<!-- majordomus:unrun -->
```bash
scripts/site-deploy
```')"
MJ_ROOT="$t" expect_exit 10 "$GATE"
expect_grep 'carries no reason'

# 6. what the gate must NOT refuse, each for its own reason:
#    a diagram, recorded output, data, a placeholder, and somebody else'"'"'s tool
t="$(fixture F '```mermaid
graph TD; a-->b;
```

```console
$ majordomus doctor
doctor: 0 failure(s)
```

```yaml
gates: [doc-command-check]
```

```bash
majordomus start "<task>" --scope <paths>
git commit -m "message"
curl -fsSL https://example.invalid/install.sh | sh
```')"
MJ_ROOT="$t" expect_exit 0 "$GATE"

# 7. the trailing comment a document writes for its reader is not part of the command: the
#    same line with an explanation still joins to the recorded run
t="$(fixture G '```bash
majordomus doctor          # tells you which hook lines are missing
```')"
MJ_ROOT="$t" expect_exit 0 "$GATE"

# 8. known debt is carried, not forgiven, and a baseline line that matches nothing is
#    reported — a list that may only shrink cannot be allowed to rot
t="$(fixture H '```bash
majordomus worktree migrate
```')"
printf '# baseline\ndocs/GUIDE.md\tmajordomus worktree migrate\n' > "$t/.ai/repo/doc-command-baseline.txt"
MJ_ROOT="$t" expect_exit 0 "$GATE"

printf '# baseline\ndocs/GUIDE.md\tmajordomus worktree migrate\ndocs/GUIDE.md\tmajordomus gone\n' > "$t/.ai/repo/doc-command-baseline.txt"
MJ_ROOT="$t" expect_exit 10 "$GATE"
expect_grep 'matches no documented command any more'
expect_grep 'majordomus gone'

# 9. a tree with no corpora cannot decide anything, and says so rather than passing: the
#    difference between "nothing to refuse" and "could not look" is the whole point
t="$(fixture I '```bash
majordomus doctor
```')"
rm -f "$t/site/data/generated/catalogue.json" "$t/docs/generated/cli.json"
MJ_ROOT="$t" expect_exit 12 "$GATE"
