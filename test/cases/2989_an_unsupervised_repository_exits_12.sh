# majordomus-covers: doctor session check watch history plan search
# majordomus-negative: doctor session check watch history plan search
# claims: reproduce-command
# A caller has to be able to tell a healthy repository from one the tool is not installed
# in, and an exit code of 0 says the first. In a git repository with no .ai/manifest.yaml
# every command that reports on the layer exits 12 — the missing-artifact code — and names
# the file; the commands whose purpose is to work there are unchanged; and the Rust
# executable answers with the same code for the same missing layer.
. "$ROOT/test/lib.sh"
[ ! -e .ai ] || { echo "    the fixture already holds a layer"; exit 1; }

# ---------------------------------------------------------------- the shell tool: every reader exits 12 and names the file
for c in "doctor" "check" "watch" "history" "context" "search x" "session list" "session status" \
         "plan list" "plan status" "handover --list" "checkpoint --list" "finish --check" \
         "skills list" "rules list" "doctrine list"; do
  # shellcheck disable=SC2086
  expect_exit 12 "$MJ" $c
  expect_grep 'no \.ai/manifest\.yaml in '
done
# and nothing was written on the way to saying so
[ ! -e .ai ] && [ -z "$(git status --porcelain)" ] \
  || { echo "    a read-only command wrote into an unsupervised repository: $(git status --porcelain | head -3)"; exit 1; }

# ---------------------------------------------------------------- the commands that work there still do
expect_exit 0 "$MJ" version
expect_exit 0 "$MJ" help
expect_exit 0 "$MJ" init --help
[ ! -e .ai ] || { echo "    init --help installed a layer"; exit 1; }

# ---------------------------------------------------------------- the Rust executable: the same code
RB="$(rust_bin)" || rust_bin_exit $?
for c in "capabilities list" "env" "scope" "commands" "release version"; do
  # shellcheck disable=SC2086
  expect_exit 12 "$RB" $c
  expect_grep 'no Majordomus repository found'
done

# ---------------------------------------------------------------- and init is what changes the answer
"$MJ" init >/dev/null
out="$("$MJ" doctor 2>&1 || true)"
case "$out" in
  *'no .ai/manifest.yaml in '*) echo "    doctor still reports the layer missing after init"; exit 1 ;;
esac
[ -f .ai/manifest.yaml ] || { echo "    init wrote no manifest"; exit 1; }
exit 0
