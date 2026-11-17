# majordomus-covers: session knowledge rules
# majordomus-negative: session knowledge rules
# A command that reports on the layer, run in a repository that has none, exits 12 (I1998).
#
# Exit 0 is the word a script or an agent reads as healthy. Run by majordomus 0.14.0 in a
# git repository with no .ai/manifest.yaml, `doctor` and `session list` printed the line
# saying the layer is missing and exited 0, so a health check run there passed. The fixture
# this case starts in is that repository: a fresh `git init` with no layer. It holds:
#
#   1  `session list` and `doctor` exit 12 (missing artifact) and name .ai/manifest.yaml
#   2  the commands whose purpose is to work there — `init --help`, `version`, `--help`,
#      `help` — exit 0, and asking for init's usage installs nothing
#   3  the shell tool and the Rust executable exit with the same code, 12, for the same read
#      command on the same missing layer, and each names the file: `knowledge candidates`
#      and `rules show <id>`, the two read commands both programs implement with the same
#      arguments
#
# Case 02 holds doctor's exit outside a supervised repository among its other findings;
# this case holds the rest of the issue's criteria. Skips itself when there is neither cargo
# nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?

[ ! -e .ai/manifest.yaml ] || { echo "    the fixture already carries a layer, so it proves nothing about one without"; exit 1; }
git rev-parse --git-dir >/dev/null 2>&1 || { echo "    the fixture is not a git repository"; exit 1; }

# ---------------------------------------------------------------- 1. a report on no layer
for argv in "session list" "doctor"; do
  # shellcheck disable=SC2086 # the words of the command, split on purpose
  expect_exit 12 "$MJ" $argv
  expect_grep 'no \.ai/manifest\.yaml'
  expect_grep 'run: majordomus init'
done

# ---------------------------------------------------------------- 2. what works there
expect_exit 0 "$MJ" init --help
expect_grep '^usage: majordomus init'
[ ! -e .ai ] || { echo "    'init --help' created .ai/: asking for usage installed the layer"; exit 1; }
expect_exit 0 "$MJ" version
expect_grep '^majordomus [0-9]+\.[0-9]+\.[0-9]+'
expect_exit 0 "$MJ" --help
expect_grep 'exit codes: .*12 missing artifact'
expect_exit 0 "$MJ" help

# ---------------------------------------------------------------- 3. both programs agree
for argv in "knowledge candidates" "rules show project.alpha"; do
  # shellcheck disable=SC2086
  expect_exit 12 "$MJ" $argv
  expect_grep '\.ai/manifest\.yaml'
  # shellcheck disable=SC2086
  expect_exit 12 "$RB" $argv
  expect_grep '\.ai/manifest\.yaml'
done
[ ! -e .ai ] || { echo "    a read command created .ai/ in a repository that had none"; exit 1; }

echo "    with no layer: session list and doctor exit 12 naming the manifest; init --help, version and help exit 0 and install nothing; the shell tool and the Rust executable both exit 12 for knowledge candidates and rules show"
