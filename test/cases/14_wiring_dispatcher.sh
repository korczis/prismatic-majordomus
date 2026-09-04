# majordomus-covers: doctor
# majordomus-negative: doctor
. "$ROOT/test/lib.sh"
# A git hook is commonly a dispatcher that runs every executable in <hook>.d/. The
# invocation then lives in one of those files, and doctor has to follow it there.
"$MJ" init >/dev/null
"$MJ" update >/dev/null
git config core.hooksPath .githooks
mkdir -p .githooks/pre-commit.d .githooks/pre-push.d

for h in pre-commit pre-push; do
  cat > ".githooks/$h" <<'HOOK'
#!/usr/bin/env bash
set -uo pipefail
DIR="$(git rev-parse --show-toplevel)/.githooks/$(basename "$0").d"
rc=0
for f in "$DIR"/*; do [ -x "$f" ] || continue; "$f" || rc=1; done
exit "$rc"
HOOK
  chmod +x ".githooks/$h"
done

# a dispatcher that dispatches to nothing is not wiring
expect_exit 10 "$MJ" doctor
expect_grep 'FAIL wiring +doctor-on-commit — majordomus doctor is not invoked by .githooks/pre-commit or anything in .githooks/pre-commit.d/'

# a subhook that is not executable is skipped by the dispatcher, so it is not wiring
printf '#!/usr/bin/env bash\n%s doctor || exit $?\n' "$MJ" > .githooks/pre-commit.d/10-majordomus.sh
expect_exit 10 "$MJ" doctor
expect_grep 'FAIL wiring +doctor-on-commit — .githooks/pre-commit.d/10-majordomus.sh invokes it but is not executable'

# executable, exit code honoured: wired, and doctor names the file that does it
chmod +x .githooks/pre-commit.d/10-majordomus.sh
expect_exit 10 "$MJ" doctor
expect_grep 'OK   wiring +doctor-on-commit — wired via .githooks/pre-commit.d/10-majordomus.sh'

# a subhook that swallows the exit code is not enforcement
printf '#!/usr/bin/env bash\n%s finish --check || true\n' "$MJ" > .githooks/pre-push.d/10-majordomus.sh
chmod +x .githooks/pre-push.d/10-majordomus.sh
expect_exit 10 "$MJ" doctor
expect_grep 'FAIL wiring +finish-on-push — .githooks/pre-push.d/10-majordomus.sh invokes it but swallows the exit code'

# both wired properly: nothing left to report
printf '#!/usr/bin/env bash\n%s finish --check || exit $?\n' "$MJ" > .githooks/pre-push.d/10-majordomus.sh
chmod +x .githooks/pre-push.d/10-majordomus.sh
expect_exit 0 "$MJ" doctor
expect_grep 'doctor: 0 failure'

# the dispatcher itself is still a valid place to put the invocation
rm -f .githooks/pre-commit.d/10-majordomus.sh
printf '#!/usr/bin/env bash\n%s doctor || exit $?\n' "$MJ" > .githooks/pre-commit
chmod +x .githooks/pre-commit
expect_exit 0 "$MJ" doctor
expect_grep 'OK   wiring +doctor-on-commit — wired via .githooks/pre-commit$'
