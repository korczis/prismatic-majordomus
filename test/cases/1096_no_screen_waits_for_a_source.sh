# majordomus-covers: none
# No screen of the UI waits for a source (I2486, ADR 0142,
# project.no-screen-waits-for-a-source):
#
#   no handler on the request   nothing under src/ui but source.rs calls `execute`, outside
#                               a test module: a screen asks for readings
#   a moved repository          a served screen answers after the repository changed with
#                               what it has, marked as catching up and said to predate the
#                               change, instead of waiting for its sources or the reload
#   and then it is current      read again, the same screen shows the changed repository and
#                               nothing on it is catching up any more
#
# The server runs with MAJORDOMUS_READING_PATIENCE_MS=0, so that "the readings it has" is
# what every source answers after a change and the case asserts no timing.
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the other Rust cases do.
. "$ROOT/test/lib.sh"

# ---------------------------------------------------------------- no handler on the request
found=0
for file in "$ROOT"/apps/majordomus-cli/src/ui/*.rs; do
  [ "$(basename "$file")" != source.rs ] || continue
  # a file's test module is its last item: what precedes `#[cfg(test)]` is what is served
  hits="$(sed '/^#\[cfg(test)\]/,$d' "$file" | grep -n '\.execute(' || true)"
  [ -z "$hits" ] || {
    echo "    ${file#"$ROOT"/} calls a handler on the path of a request:"
    printf '%s\n' "$hits" | sed 's/^/    | /'
    found=1
  }
done
[ "$found" = 0 ] || exit 1
# a screen's source asks for a reading, owned or shared (`ctx.read`, `read_shared(ctx, ...)`)
grep -qE 'ctx\.read\(|read_shared\(ctx,' "$ROOT/apps/majordomus-cli/src/ui/source.rs" \
  || { echo "    ui/source.rs asks for no reading: the scan above would hold nothing"; exit 1; }

# ---------------------------------------------------------------- a moved repository
RB="$(rust_bin)" || rust_bin_exit $?
[ -x "$RB" ] || { echo "    the build produced no executable at $RB"; exit 1; }
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
MAJORDOMUS_READING_PATIENCE_MS=0; export MAJORDOMUS_READING_PATIENCE_MS
# the fixture's server listens on loopback whatever the machine's own default host is
unset MAJORDOMUS_HTTP_HOST MAJORDOMUS_CAPABILITY_CACHE

"$MJ" init >/dev/null
git add -A >/dev/null && git commit -qm install

SRV=""
trap 'serve_down' EXIT
serve_up serve.out serve.err || exit 1
get() { curl -s --max-time 120 "$U$1"; }
# a page is matched from a here-string, never a pipe: under pipefail, grep -q exiting on its
# first match breaks printf's pipe and the pipeline fails, so a match read as a miss (I4226)

# the first reading of every source is waited for: there is nothing else to show
first="$(get /ui)"
grep -q 'mj-ui-source' <<<"$first" || { echo "    the dashboard names no source"; exit 1; }
if grep -q 'data-catching-up' <<<"$first"; then
  echo "    a first reading was not waited for: the first page is already catching up"; exit 1
fi
before="$(git rev-parse HEAD | cut -c1-10)"
grep -q "$before" <<<"$first" || { echo "    the dashboard does not show the commit it was read at ($before)"; exit 1; }

git commit -q --allow-empty -m 'the repository moves'
after="$(git rev-parse HEAD | cut -c1-10)"

moved="$(get /ui)"
grep -q 'data-catching-up' <<<"$moved" \
  || { echo "    after the repository moved, nothing on the dashboard is marked as catching up"; exit 1; }
# which of the two it says depends on whether the server's reload of the repository has
# finished: before, the whole screen is of the old generation; after, each reading is
grep -qE 'before the repository changed|The repository changed' <<<"$moved" \
  || { echo "    a screen from before the change does not say so"; exit 1; }

# ---------------------------------------------------------------- and then it is current
i=0
while :; do
  page="$(get /ui)"
  grep -q 'data-catching-up' <<<"$page" || break
  i=$((i+1))
  [ "$i" -lt 300 ] || { echo "    the dashboard was still catching up after 60 s"; exit 1; }
  sleep 0.2
done
grep -q "$after" <<<"$page" \
  || { echo "    once caught up, the dashboard does not show the commit the repository moved to ($after)"; exit 1; }
if grep -qE 'before the repository changed|The repository changed' <<<"$page"; then
  echo "    a current reading still says it predates the change"; exit 1
fi

echo "    no screen calls a handler; a moved repository is shown as it was, said so, and then as it is"
