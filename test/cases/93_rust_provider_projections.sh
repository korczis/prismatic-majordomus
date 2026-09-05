# majordomus-covers: none
# One renderer for the provider bootstraps. The shell tool's `update` and the Rust
# executable's `generate providers` read the same policy, the same profiles and the same
# templates, and must produce the same bytes, stamp included: a file one of them wrote is
# current to the other, a hand edit is stale to both, and a policy change moves both. When
# this case fails, two renderers have diverged and one of them is a second source of truth.
#
# Skips itself when there is neither cargo nor MAJORDOMUS_BIN, as the site cases do for zola.
. "$ROOT/test/lib.sh"
S="$(mktemp -d "${TMPDIR:-/tmp}/mj93.XXXXXX")"; trap 'rm -rf "$S"' EXIT
RB="$(rust_bin)" || rust_bin_exit $?
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE

"$MJ" init >/dev/null
# before the shell tool renders them, the Rust executable reports every declared target missing
expect_exit 10 "$RB" generate providers --check
expect_grep 'AGENTS.md \(missing\)'
expect_grep 'GEMINI.md \(missing\)'
expect_exit 0 "$MJ" update
git add -A >/dev/null && git commit -qm install

# --- what the shell tool wrote is current to the Rust executable, byte for byte
expect_exit 0 "$RB" generate providers --check
expect_grep '^OK   generated   AGENTS.md'
expect_grep '^OK   generated   CLAUDE.md'
expect_grep '^OK   generated   GEMINI.md'
expect_grep 'generate --check: in sync'
shell_agents="$(shasum -a 256 AGENTS.md | cut -d' ' -f1)"

# --- a hand edit is stale to both
printf '\nA rule of my own.\n' >> AGENTS.md
expect_exit 10 "$RB" generate providers --check
expect_grep 'AGENTS.md \(differs\)'
doctor_out="$("$MJ" doctor 2>&1 || true)"
grep -q '^FAIL projection  AGENTS.md' <<<"$doctor_out" || { echo "    the shell doctor did not see the hand edit"; echo "$doctor_out"; exit 1; }

# --- the Rust executable repairs it to the very bytes the shell tool wrote
expect_exit 0 "$RB" generate providers
[ "$(shasum -a 256 AGENTS.md | cut -d' ' -f1)" = "$shell_agents" ] || { echo "    Rust rewrote AGENTS.md to different bytes than the shell tool"; exit 1; }
doctor_out="$("$MJ" doctor 2>&1 || true)"
grep -q '^OK   projection  AGENTS.md' <<<"$doctor_out" || { echo "    the shell doctor does not accept the Rust-written AGENTS.md"; echo "$doctor_out"; exit 1; }

# --- a policy change: the shell tool re-renders, the Rust executable agrees on the new stamp
sed -i.bak 's/^  checkpoint_interval_default: .*/  checkpoint_interval_default: 30m/' .ai/repo/policy.yaml && rm -f .ai/repo/policy.yaml.bak
expect_exit 10 "$RB" generate providers --check
expect_exit 0 "$MJ" update
grep -q 'is `30m`' AGENTS.md || { echo "    update did not render the new interval"; exit 1; }
expect_exit 0 "$RB" generate providers --check
[ "$(shasum -a 256 AGENTS.md | cut -d' ' -f1)" != "$shell_agents" ] || { echo "    the policy change did not move the stamp"; exit 1; }

# --- and the other way round: Rust writes, the shell tool's doctor is content
sed -i.bak 's/^  checkpoint_interval_default: .*/  checkpoint_interval_default: 45m/' .ai/repo/policy.yaml && rm -f .ai/repo/policy.yaml.bak
expect_exit 0 "$RB" generate providers
doctor_out="$("$MJ" doctor 2>&1 || true)"
grep -q '^OK   projection  AGENTS.md' <<<"$doctor_out" && grep -q '^OK   projection  GEMINI.md' <<<"$doctor_out" \
  || { echo "    the shell doctor does not accept the Rust-written projections"; echo "$doctor_out"; exit 1; }
expect_exit 0 "$MJ" update --dry-run
expect_grep '^unchanged AGENTS.md'
expect_grep '^unchanged CLAUDE.md'
