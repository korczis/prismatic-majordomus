# majordomus-covers: none
# A derivation must not write from a tree the tool has already said is incomplete. The index
# is deliberately tolerant — a file it cannot place becomes a diagnostic and the build goes
# on, so one broken document does not make the repository unreadable — but that tolerance is
# for reading, not for writing down what the repository is. This case gives one identity two
# claimants, which excludes both, and holds the derivation to refusing rather than producing
# projections that silently lack them.
. "$ROOT/test/lib.sh"
RB="$(rust_bin)" || rust_bin_exit $?
# the distribution the executable reads its kinds and schemas from; the fixture repo has
# none of its own, which is the legitimate case the share guard permits
MAJORDOMUS_SHARE="$ROOT/share"; export MAJORDOMUS_SHARE
"$MJ" init >/dev/null; "$MJ" update >/dev/null
mkdir -p lib && echo a > lib/a && git add -A >/dev/null && git commit -qm base

# a decision, and then the same identity a second time
adr() {
  mkdir -p .ai/repo/adrs
  printf -- '---\nschema: adr/v1\nid: adr-%s\nkind: adr\ntitle: %s\nstatus: proposed\ndate: 2026-01-01\ntags: [probe]\n---\n\n# %s. %s\n\n## Context\n\nProbe.\n\n## Decision\n\nProbe.\n' \
    "$1" "$2" "${1#000}" "$2" > ".ai/repo/adrs/$1-$3.md"
  git add ".ai/repo/adrs/$1-$3.md" >/dev/null
}
adr 0091 "The first" first
expect_exit 0 "$RB" generate allow --repo . --strict

# the same identity claimed twice: both claimants are excluded from the index
adr 0091 "The second" second
expect_exit 0 "$RB" generate allow --repo .            # tolerant: it still writes
expect_exit 10 "$RB" generate allow --repo . --strict  # strict: it refuses
expect_grep 'error\(s\); refusing to serve under --strict'

# the derivation and its check are the invocations that must be strict, so that a document
# the index dropped cannot become a committed projection that omits it
grep -q -- '--strict' "$ROOT/scripts/derive" \
  || { echo "    scripts/derive does not pass --strict; a degraded index would derive silently"; exit 1; }
grep -q -- '--strict' "$ROOT/scripts/derive-check" \
  || { echo "    scripts/derive-check does not pass --strict"; exit 1; }

# and with the duplicate gone it derives again
git rm -q -f .ai/repo/adrs/0091-second.md >/dev/null
expect_exit 0 "$RB" generate allow --repo . --strict

# ---------------------------------------------------------------- one pass, every gap
# The companion failure to a silent exit code is a loud one that arrives six times. Adding a
# public command on 2026-09-09 cost six derivations, because each refusal named one missing
# piece and all six were knowable at once. This holds the check that says them together.
#
# It examines a COPY of the checkout through --root, so a case running beside seven other
# sessions never edits share/commands.yaml in the tree they are all working in.
expect_exit 0 "$ROOT/scripts/ci/command-furnished"

T="$(mktemp -d "${TMPDIR:-/tmp}/mj104r.XXXXXX")"
mkdir -p "$T/share" "$T/docs/claims" "$T/test/cases" "$T/test/fixtures/commands"
cp "$ROOT/share/commands.yaml" "$T/share/commands.yaml"
cp "$ROOT/docs/CLI.md" "$T/docs/CLI.md"
cp "$ROOT/docs/CLAIMS.yaml" "$T/docs/CLAIMS.yaml"
cp "$ROOT"/docs/claims/*.md "$T/docs/claims/" 2>/dev/null || true
cp "$ROOT"/test/cases/*.sh "$T/test/cases/" 2>/dev/null || true
cp "$ROOT"/test/fixtures/commands/*.json "$T/test/fixtures/commands/" 2>/dev/null || true
expect_exit 0 "$ROOT/scripts/ci/command-furnished" --root "$T"

# a public command with nothing behind it reports every requirement, not the first one
python3 - "$T" <<'PY2'
import io, sys
p = sys.argv[1] + '/share/commands.yaml'
s = io.open(p, encoding='utf-8').read()
probe = """  - id: probe
    title: probe
    summary: A probe command with nothing behind it.
    category: task
    stage: work
    visibility: public
    class: read-only
    requires_repository: true
    requires_installed: true
    requires_task: none
    json: false
    syntax: majordomus probe
    reads: []
    writes: []
    exit_codes: [0]

"""
io.open(p, 'w', encoding='utf-8').write(s.replace('  - id: skills\n', probe + '  - id: skills\n'))
PY2
expect_exit 10 "$ROOT/scripts/ci/command-furnished" --root "$T" probe
expect_grep 'no section in docs/CLI.md'
expect_grep 'no fixture at test/fixtures/commands/probe.json'
expect_grep 'declares no demo'
expect_grep 'majordomus-covers: probe'
expect_grep 'majordomus-negative: probe'
expect_grep '5 requirement\(s\) unmet'
rm -rf "$T"
