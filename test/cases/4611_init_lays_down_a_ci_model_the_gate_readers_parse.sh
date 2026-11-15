# majordomus-covers: init
# majordomus-negative: init
#
# `init --ci` lays down a CI model both gate readers parse (I4611). An adopter's `check` used
# to say that the repository "declares no CI model ... nothing can be planned" and nothing
# more: the skeleton had no ci/ directory, so init laid nothing down, and the shape of the
# model had to be read out of the crate. The model is offered rather than seeded by default,
# because a declared model changes what `finish` means (an unreadable verdict refuses
# completed, case 578); the finding a repository without one gets names the command.
. "$ROOT/test/lib.sh"

# A plain init seeds no model, offers it, and the finding says how to get one.
expect_exit 0 "$MJ" init
expect_grep '^ci: no CI model laid down; majordomus init --extend --ci'
"$MJ" update >/dev/null
[ ! -e .ai/repo/ci/gates.yaml ] || { printf '    a plain init seeded a CI model\n'; exit 1; }
mkdir -p lib && echo a > lib/a && git add -A && git commit -qm base
"$MJ" start "t1" --scope lib >/dev/null
echo b >> lib/a
expect_exit 0 "$MJ" check
expect_grep 'declares no CI model at \.ai/repo/ci/gates\.yaml.*majordomus init --extend --ci'

# --extend --ci adds the model and its directory contract, and says so.
expect_exit 0 "$MJ" init --extend --ci
expect_grep '\.ai/repo/ci/gates\.yaml'
expect_file .ai/repo/ci/gates.yaml
expect_file .ai/repo/ci/README.md
cmp -s "$ROOT/share/skeleton/ai/repo/ci/gates.yaml" .ai/repo/ci/gates.yaml \
  || { printf '    the seeded model is not the skeleton\n'; exit 1; }
# It names a hosted job's command as landing evidence by its lane, as an example, and says
# what the model does not model.
expect_grep '^ *#.*job: test$' .ai/repo/ci/gates.yaml
expect_grep '^ *#.*lane: feedback$' .ai/repo/ci/gates.yaml
expect_grep '^#.*[Nn]ot modelled' .ai/repo/ci/gates.yaml
git add -A && git commit -qm 'the CI model'

# lib/gates.sh parses it: the gate it declares is known, and a run of it is recorded.
expect_exit 0 "$MJ" evidence --gate skills-verify --exit 0 --command "majordomus skills verify"
expect_grep 'evidence: gate skills-verify reported exit 0'
expect_grep '"event":"task.gate".*"gate":"skills-verify","exit":0' .ai/local/state/ledger.jsonl
expect_exit 2 "$MJ" evidence --gate no-such-gate --exit 0
expect_grep "it declares: skills-verify"
expect_no_grep 'declares no CI model'

# The executable's reader accepts it too (GateModel::load refuses a model with no gates, an
# unknown lane or a bad version), when an executable is at hand.
if [ -n "${MAJORDOMUS_BIN:-}" ] && [ -x "${MAJORDOMUS_BIN:-}" ] && command -v jq >/dev/null 2>&1; then
  out="$(MAJORDOMUS_SHARE="$ROOT/share" "$MAJORDOMUS_BIN" run gates.completion --input '{}' \
    --quiet --format json --repo "$PWD" 2>gates.err)" \
    || { printf '    gates.completion refused the seeded model: %s\n' "$(tail -n 5 gates.err)"; exit 1; }
  printf '%s' "$out" | jq -e '.output.gates[] | select(.id == "skills-verify" and .required)' \
    >/dev/null || { printf '    gates.completion does not plan the seeded gate: %s\n' "$out"; exit 1; }
  rm -f gates.err
fi

# A model the repository wrote is never overwritten.
printf 'version: 1\ngates:\n  - id: mine\n    job: j\n    runs: make test\nclasses: []\n' > .ai/repo/ci/gates.yaml
expect_exit 0 "$MJ" init --extend --ci
expect_grep '^  - id: mine$' .ai/repo/ci/gates.yaml
