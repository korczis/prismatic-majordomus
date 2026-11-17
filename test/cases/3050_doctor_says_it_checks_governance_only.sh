# majordomus-covers: none
# `majordomus doctor` says that it checks governance only, and names where the runtime is
# read (I2707).
#
# The top-level doctor is the governance doctor of lib/doctor.sh: policy, profiles, the
# wiring of every declared enforcement, projections, records and retention. It reads nothing
# about the checkout's server, its lease or the mesh, and its name suggests that it does: a
# worker whose server was stale ran `majordomus doctor`, read a clean report and believed
# the runtime healthy. I2157 made GET /api/v1/ready refuse a lost lease and gave health.report
# a mesh check; this case holds the other half, that every surface describing doctor says
# the same two things together:
#
#   1  `majordomus doctor --help` exits 0, prints its usage, says it checks governance only,
#      and names `majordomus serve status`, `majordomus mesh doctor`, GET /api/v1/health and
#      GET /api/v1/ready as where the runtime is read
#   2  the shell tool's usage lists doctor as governance only
#   3  share/commands.yaml, which the reference and the site are generated from, says it in
#      doctor's summary and names the runtime readers in its note
#   4  docs/CLI.md's doctor section says it and names the same readers
#
# A surface that drops the sentence fails here, naming the surface.
. "$ROOT/test/lib.sh"

runtime_readers='majordomus serve status|majordomus mesh doctor|/api/v1/health|/api/v1/ready'

# ---------------------------------------------------------------- 1. doctor's own help
expect_exit 0 "$MJ" doctor --help
expect_grep 'usage: majordomus doctor'
expect_grep 'governance only'
for reader in 'majordomus serve status' 'majordomus mesh doctor' 'GET /api/v1/health' 'GET /api/v1/ready'; do
  expect_grep "$reader"
done

# ---------------------------------------------------------------- 2. the shell usage
expect_exit 0 "$MJ" --help
grep -E '^  doctor +' <<<"$LAST_OUT" | grep -q 'governance only' \
  || { echo "    majordomus --help does not list doctor as governance only:"; grep -E '^  doctor ' <<<"$LAST_OUT" | sed 's/^/      /'; exit 1; }

# ---------------------------------------------------------------- 3. the command registry
entry="$(awk '/^  - id: doctor$/ { on = 1; print; next } on && /^  - id: / { exit } on { print }' "$ROOT/share/commands.yaml")"
[ -n "$entry" ] || { echo "    share/commands.yaml has no doctor entry"; exit 1; }
printf '%s\n' "$entry" | grep -E '^    summary: ' | grep -q 'governance only' \
  || { echo "    share/commands.yaml: doctor's summary does not say it checks governance only"; exit 1; }
for reader in 'majordomus serve status' 'majordomus mesh doctor' '/api/v1/health' '/api/v1/ready'; do
  printf '%s\n' "$entry" | grep -E '^    note: ' | grep -qF "$reader" \
    || { echo "    share/commands.yaml: doctor's note does not name $reader"; exit 1; }
done

# ---------------------------------------------------------------- 4. the reference
section="$(awk '/^## `majordomus doctor`$/ { on = 1; next } on && /^## / { exit } on { print }' "$ROOT/docs/CLI.md")"
[ -n "$section" ] || { echo "    docs/CLI.md has no majordomus doctor section"; exit 1; }
printf '%s\n' "$section" | grep -q 'governance only' \
  || { echo "    docs/CLI.md: the doctor section does not say it checks governance only"; exit 1; }
for reader in 'majordomus serve status' 'majordomus mesh doctor' '/api/v1/health' '/api/v1/ready'; do
  printf '%s\n' "$section" | grep -qF "$reader" \
    || { echo "    docs/CLI.md: the doctor section does not name $reader"; exit 1; }
done

echo "    doctor --help, the shell usage, share/commands.yaml and docs/CLI.md say doctor checks governance only and name where the runtime is read ($runtime_readers)"
