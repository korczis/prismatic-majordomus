# majordomus-covers: none
# An installed Majordomus runs the commands its own bootstraps tell a worker to run.
#
# Every adopting repository's AGENTS.md, CLAUDE.md and GEMINI.md are generated from
# share/providers/*.tmpl, and those name `majordomus worktree`, `majordomus worktree create
# <branch>` and `majordomus worktree migrate` — commands of the Rust executable. An installed
# tree is bin/, lib/, libexec/ and share/ with no docs/ (scripts/release-package), so the shell
# tool could not read the clap projection and answered "unknown command: worktree": an
# instruction the tool itself wrote could not be followed (seen in OSCILLA on 0.10 to 0.12).
#
# The installed layout is built here from this checkout, with a stand-in for the shipped
# executable that answers `help <command>` the way clap does and echoes what it was run with.
. "$ROOT/test/lib.sh"
unset MAJORDOMUS_BIN MAJORDOMUS_SHARE
I="$T/installed"
mkdir -p "$I/bin" "$I/lib" "$I/libexec" "$I/share"
cp "$ROOT/bin/majordomus" "$ROOT/bin/majordomus-cli" "$I/bin/"
cp "$ROOT"/lib/* "$I/lib/"
cp -R "$ROOT/share/." "$I/share/"
cat > "$I/libexec/majordomus-cli" <<'STUB'
#!/bin/sh
if [ "$1" = help ]; then
  case "$2" in worktree|generate) exit 0 ;; *) echo "error: unrecognized subcommand '$2'" >&2; exit 2 ;; esac
fi
echo "executable ran: $*"
STUB
chmod +x "$I/bin/majordomus" "$I/bin/majordomus-cli" "$I/libexec/majordomus-cli"
[ ! -e "$I/docs" ] || { echo "    the installed layout must carry no docs/"; exit 1; }

# the commands every bootstrap names reach the executable, arguments unchanged
expect_exit 0 "$I/bin/majordomus" worktree
expect_grep '^executable ran: worktree$'
expect_exit 0 "$I/bin/majordomus" worktree create feature/x
expect_grep '^executable ran: worktree create feature/x$'
expect_exit 0 "$I/bin/majordomus" worktree migrate --plan
expect_grep '^executable ran: worktree migrate --plan$'
expect_no_grep 'unknown command'

# the shell tool's own commands are still its own
expect_exit 0 "$I/bin/majordomus" version
expect_grep '^majordomus '

# a command neither program has is still unknown, and nothing is run for it
expect_exit 2 "$I/bin/majordomus" nonsense
expect_grep 'unknown command: nonsense'
expect_no_grep 'executable ran'

# and the shared bootstraps an adopter is generated from cite nothing only this repository has
for t in "$ROOT"/share/providers/*.tmpl; do
  if grep -nE 'project\.worktree-topology|docs/WORKTREES\.md' "$t"; then
    echo "    $(basename "$t") cites a rule or document only the Majordomus repository has"; exit 1
  fi
done
