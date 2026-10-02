# majordomus-covers: none
# An installed tool finds its own executable, not one in the repository it is working on.
#
# lib/common.sh keeps two roots apart: MJ_HOME, the distribution the tool runs from, and
# MJ_ROOT, the repository it supervises. Several callers of lib/rust_bin.sh passed $MJ_ROOT
# where they meant $MJ_HOME, so the shell tool looked for <repository>/libexec/majordomus-cli
# and <repository>/apps/majordomus-cli/target. In this source checkout the two roots are one
# directory and nothing could tell; in a packaged install
# (~/.local/share/majordomus/versions/<v>/{bin,lib,libexec,share}) serving any other
# repository the executable was never found, and every Rust-backed feature of the shell tool
# degraded in silence. An owner found it from an adopting repository on 0.10.0.
#
# So the fixture is a real packaged layout in a directory of its own, with a stub at
# <dist>/libexec/majordomus-cli that records every call, and a repository elsewhere; and from
# inside that repository, with MAJORDOMUS_BIN unset, two paths must reach the stub:
#
#   doctor  -> mj_report_reasoning asks it for the advisors
#   check   -> the completion-gates validator asks it for gates.completion
#
# and the structural half refuses any lib/ call that hands the repository's root to the
# functions that choose the executable, so a new caller cannot reintroduce the defect at a
# site this case does not walk.
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "no jq: doctor and the gates validator read the executable's answer with jq"

# ---------------------------------------------------------------- structural
bad="$(grep -nE 'mj_rust_(bin|stale) "\$\{?MJ_ROOT\}?"|mj_rust_reasoning_report "\$\{?MJ_ROOT\}?"\)' "$ROOT"/lib/*.sh || true)"
if [ -n "$bad" ]; then
  printf '    these calls choose the executable by the repository root, not the tool'"'"'s ($MJ_HOME):\n%s\n' "$bad"
  exit 1
fi

# ---------------------------------------------------------------- the packaged layout
DIST="$(mktemp -d "${TMPDIR:-/tmp}/mj744.dist.XXXXXX")"
trap 'rm -rf "$DIST"' EXIT
mkdir -p "$DIST/bin" "$DIST/lib" "$DIST/libexec" "$DIST/share"
cp "$ROOT/bin/majordomus" "$ROOT/bin/majordomus-mcp" "$ROOT/bin/majordomus-cli" "$DIST/bin/"
cp "$ROOT"/lib/* "$DIST/lib/"
cp -R "$ROOT/share/." "$DIST/share/"
cat > "$DIST/RELEASE.json" <<'JSON'
{
  "schema": "majordomus/build/v1",
  "version": "0.0.0",
  "tag": "v0.0.0",
  "target": "test",
  "commit": "test",
  "built_at": "1970-01-01T00:00:00Z"
}
JSON
CALLS="$DIST/calls.log"
cat > "$DIST/libexec/majordomus-cli" <<STUB
#!/bin/sh
# the packaged executable, as far as this case needs one: it says it was asked
printf '%s\n' "\$*" >> "$CALLS"
case "\$*" in
  --version*) echo 'majordomus 0.0.0-stub' ;;
  'reasoning advisors'*) echo '{"mode":{"mode":"stub-744","source":"packaged"},"available":0,"advisors":[]}' ;;
  'reasoning check'*) echo '{"ok":true,"checks":["stub-744"],"findings":[]}' ;;
  'run gates.completion'*) echo '{"output":{"finishable":true,"blocking":[],"unverified":[],"gates":[]}}' ;;
esac
exit 0
STUB
chmod 0755 "$DIST/bin/majordomus" "$DIST/bin/majordomus-mcp" "$DIST/bin/majordomus-cli" "$DIST/libexec/majordomus-cli"
: > "$CALLS"
MJD() { env -u MAJORDOMUS_BIN -u MAJORDOMUS_SHARE -u MAJORDOMUS_LAUNCHER "$DIST/bin/majordomus" "$@"; }

# ---------------------------------------------------------------- a repository elsewhere
# $T is the runner's own fixture repository: not $ROOT, not under $DIST, with no libexec/
# and no crate of its own — an adopting repository.
cd "$T" || exit 1
case "$DIST/" in "$T/"*) echo "    the distribution must not sit inside the repository"; exit 1 ;; esac
[ ! -e "$T/libexec" ] && [ ! -e "$T/apps/majordomus-cli" ] || { echo "    the repository carries an executable of its own"; exit 1; }
printf '# Sample\n' > README.md
git add -A >/dev/null 2>&1 && git --no-pager commit -qm project >/dev/null 2>&1
expect_exit 0 MJD init || exit 1
expect_exit 0 MJD update || exit 1

# ---------------------------------------------------------------- doctor
MJD doctor >"$DIST/doctor.out" 2>&1 || true
LAST_OUT="$(cat "$DIST/doctor.out")"
expect_no_grep 'the executable is not built, so advisor availability' || exit 1
expect_grep 'operational, mode stub-744 \(packaged\)' || exit 1
expect_grep '^reasoning advisors ' "$CALLS" || exit 1
expect_grep "--repo $(cd "$T" && pwd -P)|--repo $T" "$CALLS" || exit 1

# ---------------------------------------------------------------- check: the gates validator
mkdir -p .ai/repo/ci
cat > .ai/repo/ci/gates.yaml <<'YAML'
version: 1
gates:
  - id: lint
    job: structure
    always: true
    runs: "true"
    summary: every script parses
YAML
git add -A >/dev/null 2>&1 && git --no-pager commit -qm gates >/dev/null 2>&1
expect_exit 0 MJD start "an installed tool" --scope README.md || exit 1
MJD check >"$DIST/check.out" 2>&1 || true
LAST_OUT="$(cat "$DIST/check.out")"
expect_no_grep 'the gate reader is not built at' || exit 1
expect_grep '^run gates\.completion ' "$CALLS" || exit 1

echo "    ok: doctor and check resolve $DIST/libexec/majordomus-cli from another repository"
