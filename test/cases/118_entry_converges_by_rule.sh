# majordomus-covers: none
# The gate of project.entry-converges: does the machinery that makes entering this
# repository converge on a ready shared server still agree with itself?
#
# Repository entry converged on a server the day ADR 0035's slice landed, and nothing made
# it a rule. A `serve ensure` in `.envrc`, a `cargo build` in the start path, a switch added
# to the policy and never taught to the schema, a shim unwired from the provider, a fourth
# `sed` parser of the lease — each is one reasonable line, and each quietly removes a piece
# of it while every other gate stays green.
#
# A gate that cannot be shown to fail is decoration, so each half is planted in a fixture
# tree — a copy of the files the gate reads, never this checkout — and the same gate must
# refuse it, naming the cause; then the fixture unmutated must be accepted. The fixture is
# what MJ_ROOT is for.
. "$ROOT/test/lib.sh"

GATE="$ROOT/scripts/ci/entry-converges"
[ -x "$GATE" ] || { echo "    scripts/ci/entry-converges is missing or not executable"; exit 1; }

# ---------------------------------------------------------------- wired into the CI model
# A gate no plan selects is prose with an exit code. The classes that can move either side
# of entry are the shell tool and its entry point, the layer (the policy and this rule), the
# distribution (the skeleton, the allow list, the schema) and the crate.
expect_grep 'id: entry-converges' "$ROOT/.ai/repo/ci/gates.yaml"
expect_grep 'scripts/ci/entry-converges' "$ROOT/.ai/repo/ci/gates.yaml"
if command -v python3 >/dev/null 2>&1 && python3 -c 'import yaml' >/dev/null 2>&1; then
  python3 - "$ROOT/.ai/repo/ci/gates.yaml" <<'PY' || exit 1
import sys, yaml
m = yaml.safe_load(open(sys.argv[1]))
for want in ("shell", "layer", "share", "rust"):
    c = next(c for c in m["classes"] if c["id"] == want)
    if "entry-converges" not in c["gates"]:
        print("    class %s does not select entry-converges" % want); sys.exit(1)
PY
else
  echo "    (no python3 with yaml; the class selection is checked by case 94 and ci-plan --check)"
fi

# ---------------------------------------------------------------- the rule resolves
RULE="$ROOT/.ai/repo/rules/project/entry-converges.v1.md"
expect_grep '^id: project\.entry-converges' "$RULE"
expect_grep '^class: blocking' "$RULE"
expect_grep 'scripts/ci/entry-converges' "$RULE"
# the rule admits what it does not decide, rather than claiming an enforcement it lacks
expect_grep 'a reviewer does' "$RULE"

# ---------------------------------------------------------------- a fixture tree
# Everything the gate reads, copied. A file the gate learns to read later and this case does
# not copy shows up as a finding on the clean fixture, which is the right way round: the
# fixture is told to be complete rather than the gate told to be lenient.
FX="$T/fixture"
for p in .envrc bin/majordomus-env lib/capture.sh lib/derive.sh lib/context.sh \
         .just/serve.just scripts/cockpit-probe .claude/hooks/majordomus-session-start \
         .ai/repo/policy.yaml share/skeleton/policy.yaml share/allow/policy.txt \
         share/schemas/majordomus/policy/policy.v1.schema.json \
         apps/majordomus-cli/src/cli.rs apps/majordomus-cli/src/lease.rs \
         apps/majordomus-cli/src/commands/serve.rs; do
  [ -f "$ROOT/$p" ] || { echo "    the checkout has no $p; the gate reads it"; exit 1; }
  mkdir -p "$FX/$(dirname "$p")"
  cp "$ROOT/$p" "$FX/$p"
done

gate() { MJ_ROOT="$FX" "$GATE" --no-suite; }

# the fixture as copied is this repository, so it passes
expect_exit 0 gate
expect_grep 'entry-converges: entry starts nothing'
expect_no_grep '^FAIL'

# one restorer for every mutation: keep a pristine copy, put it back afterwards
save() { cp "$FX/$1" "$T/pristine"; }
restore() { cp "$T/pristine" "$FX/$1"; }

# ---------------------------------------------------------------- 1. entry by a shell serves
# The change the mandate for all of this asked for by name, and the one ADR 0003 refuses: a
# shell is not a client, and the file a shell evaluates on entry may not start a server.
save .envrc
printf 'bin/majordomus-cli serve ensure --idle 900\n' >> "$FX/.envrc"
expect_exit 10 gate
expect_grep 'starts a server on entry'
expect_grep 'entry by a shell reports and does not serve'
restore .envrc

# a build on the hot path of every `cd`
printf '  cargo build --release\n' >> "$FX/.envrc"
expect_exit 10 gate
expect_grep 'starts a build or a daemon'
restore .envrc

# and the same file naming `just build` inside a message is not a command: the adapter this
# repository ships says exactly that, and the gate must not be reading prose
expect_grep 'run .just build.' "$FX/bin/majordomus-env"
expect_exit 0 gate

save bin/majordomus-env
printf '"$bin" serve --idle 60 &\n' >> "$FX/bin/majordomus-env"
expect_exit 10 gate
expect_grep 'backgrounds a process'
restore bin/majordomus-env

# ---------------------------------------------------------------- 2. the switch is declared
# A key in the policy the schema does not know is refused at run time, so the policy stops
# parsing for everybody: the four sites are declared together or not at all.
save share/schemas/majordomus/policy/policy.v1.schema.json
sed 's/ensure_server_on_start/renamed_key/g' "$T/pristine" > "$FX/share/schemas/majordomus/policy/policy.v1.schema.json"
expect_exit 10 gate
expect_grep 'the policy schema .* does not declare session.ensure_server_on_start'
restore share/schemas/majordomus/policy/policy.v1.schema.json

save share/skeleton/policy.yaml
sed 's/ensure_server_on_start/renamed_key/g' "$T/pristine" > "$FX/share/skeleton/policy.yaml"
expect_exit 10 gate
expect_grep 'the policy a new repository is written from .* does not declare'
restore share/skeleton/policy.yaml

# ---------------------------------------------------------------- 3. the start event is wired
save .claude/hooks/majordomus-session-start
sed 's/capture session/capture prompt/' "$T/pristine" > "$FX/.claude/hooks/majordomus-session-start"
expect_exit 10 gate
expect_grep "does not dispatch 'capture session'"
restore .claude/hooks/majordomus-session-start

# unwiring the shim from the policy is what makes doctor stop reconciling it
save .ai/repo/policy.yaml
sed 's/provider-hook:claude-code:session/manual/' "$T/pristine" > "$FX/.ai/repo/policy.yaml"
expect_exit 10 gate
expect_grep 'declares no enforcement entry wired by'
restore .ai/repo/policy.yaml

save lib/capture.sh
sed 's/serve ensure/serve status/' "$T/pristine" > "$FX/lib/capture.sh"
expect_exit 10 gate
expect_grep "never runs 'serve ensure'"
restore lib/capture.sh

printf 'mj_capture_build() { cargo build --release; }\n' >> "$FX/lib/capture.sh"
expect_exit 10 gate
expect_grep 'runs a builder; nothing on entry may build'
restore lib/capture.sh

# ---------------------------------------------------------------- 4. one author for the briefing
save lib/derive.sh
printf 'mj_derive_server() { jq -r .url .ai/local/state/mcp/server.json; }\n' >> "$FX/lib/derive.sh"
expect_exit 10 gate
expect_grep 'reads the lease itself'
restore lib/derive.sh

printf 'mj_derive_server() { curl -fsS "$url/api/v1/server"; }\n' >> "$FX/lib/derive.sh"
expect_exit 10 gate
expect_grep 'second opinion'
restore lib/derive.sh

# ---------------------------------------------------------------- 5. one reading of the lease
mkdir -p "$FX/scripts"
cat > "$FX/scripts/whereis" <<'EOF'
#!/bin/sh
sed -n 's/.*"url":"\([^"]*\)".*/\1/p' .ai/local/state/mcp/server.json
EOF
expect_exit 10 gate
expect_grep 'parsed by hand outside the typed reader'
rm -f "$FX/scripts/whereis"

printf 'const P: &str = "state/mcp/server.json";\nfn u() -> &str { "\\"url\\"" }\n' > "$FX/apps/majordomus-cli/src/other.rs"
expect_exit 10 gate
expect_grep 'parsed in the crate outside src/lease.rs'
rm -f "$FX/apps/majordomus-cli/src/other.rs"

# ---------------------------------------------------------------- 6. a bounded life
save apps/majordomus-cli/src/cli.rs
sed 's/^pub const DEFAULT_IDLE_SECONDS: u64 = [0-9]*;$/pub const DEFAULT_IDLE_SECONDS: u64 = 0;/' \
  "$T/pristine" > "$FX/apps/majordomus-cli/src/cli.rs"
expect_exit 10 gate
expect_grep 'must end when no peer has been attached'
restore apps/majordomus-cli/src/cli.rs

save apps/majordomus-cli/src/commands/serve.rs
sed 's/"--idle"/"--quiet"/' "$T/pristine" > "$FX/apps/majordomus-cli/src/commands/serve.rs"
expect_exit 10 gate
expect_grep 'spawns a server without --idle'
restore apps/majordomus-cli/src/commands/serve.rs

# ---------------------------------------------------------------- and clean again
# Every restore put back what it took: if one did not, this is where it is seen.
expect_exit 0 gate
expect_no_grep '^FAIL'

# an unusable invocation is an error, never a pass
expect_exit 2 env MJ_ROOT="$FX" "$GATE" --nonsense

echo "    the entry gate refuses each thing the rule forbids, and accepts the tree that does not"
