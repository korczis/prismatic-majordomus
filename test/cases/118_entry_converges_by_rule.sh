# majordomus-covers: none
# The gate of project.entry-converges: does the machinery that makes entering this
# repository converge on a ready shared server still agree with itself?
#
# Repository entry converged on a server the day ADR 0035's slice landed, and nothing made
# it a rule. A `serve ensure` in `.envrc`, a `cargo build` in the start path, a switch added
# to the policy and never taught to the schema, a shim unwired from the provider, a `curl`
# for the server's standing beside the line the start event was handed — each is one
# reasonable line, and each quietly removes a piece of it while every other gate stays green.
#
# ADR 0043 widened what the rule protects rather than narrowing it. Entry by a shell now
# converges too, through one bootstrap command, so the things that can be quietly undone now
# also include: a second call in the entry file, a `--wait` that turns entering a directory
# into a wait for a bind, a shell entry path that stops reading the switch or stops honouring
# MAJORDOMUS_RUNTIME, an adapter that ensures a runtime from an executable older than its
# sources, a second implementation of the convergence beside the one `serve ensure` uses, and
# a budget dropped from any of the four places a policy key lives.
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
RULE="$ROOT/.ai/repo/rules/project/entry-converges.v2.md"
expect_grep '^id: project\.entry-converges' "$RULE"
expect_grep '^class: blocking' "$RULE"
expect_grep 'scripts/ci/entry-converges' "$RULE"
# the rule admits what it does not decide, rather than claiming an enforcement it lacks
expect_grep 'a reviewer does' "$RULE"
# and it depends on the rule that owns the lease rather than restating it
expect_grep 'project\.the-lease-is-read-once' "$RULE"

# ---------------------------------------------------------------- a fixture tree
# Everything the gate reads, copied. A file the gate learns to read later and this case does
# not copy shows up as a finding on the clean fixture, which is the right way round: the
# fixture is told to be complete rather than the gate told to be lenient.
FX="$T/fixture"
for p in .envrc bin/majordomus-env lib/capture.sh lib/derive.sh \
         .claude/hooks/majordomus-session-start \
         .ai/repo/policy.yaml share/skeleton/policy.yaml share/allow/policy.txt \
         share/schemas/majordomus/policy/policy.v1.schema.json \
         apps/majordomus-cli/src/cli.rs apps/majordomus-cli/src/lease.rs \
         apps/majordomus-cli/src/commands/serve.rs apps/majordomus-cli/src/commands/env.rs; do
  [ -f "$ROOT/$p" ] || { echo "    the checkout has no $p; the gate reads it"; exit 1; }
  mkdir -p "$FX/$(dirname "$p")"
  cp "$ROOT/$p" "$FX/$p"
done

gate() { MJ_ROOT="$FX" "$GATE" --no-suite; }

# the fixture as copied is this repository, so it passes
expect_exit 0 gate
expect_grep 'entry-converges: the entry file starts nothing itself and makes one call'
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
expect_grep 'the entry file calls the bootstrap command and starts nothing itself'
restore .envrc

# ------------------------------------------------- 1b. one call, the bootstrap one, no wait
# A second call is two readings of the repository on every `cd`, and a file that chooses
# between commands has started deciding things (project.envrc-is-an-adapter@2).
printf 'eval "$(bin/majordomus-env banner)"\n' >> "$FX/.envrc"
expect_exit 10 gate
expect_grep 'makes 2 call\(s\) to the tool'
restore .envrc

# the one call being something other than the bootstrap command is the same failure, one
# step earlier: whatever else it is, the file is deciding what entering means
sed 's/enter --shell direnv/export --shell direnv --banner --bridge/' "$T/pristine" > "$FX/.envrc"
expect_exit 10 gate
expect_grep 'is not the bootstrap command'
restore .envrc

# and a wait turns entering a directory into a wait for a server to bind, which is the one
# cost ADR 0043 refused to accept in exchange for the runtime
sed 's/enter --shell direnv/enter --shell direnv --wait 20/' "$T/pristine" > "$FX/.envrc"
expect_exit 10 gate
expect_grep 'passes --wait'
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

# an adapter that names a stale executable and then ensures a runtime from it anyway: the
# server would answer with a tree that is no longer there, to every worker attached to it
sed 's/export MAJORDOMUS_RUNTIME=off//' "$T/pristine" > "$FX/bin/majordomus-env"
expect_exit 10 gate
expect_grep 'does not turn the runtime off for a stale executable'
restore bin/majordomus-env

# ------------------------------------------------- 1c. the shell's door ensures like the agent's
save apps/majordomus-cli/src/commands/env.rs
sed 's/ensure_server_on_start/some_other_key/g' "$T/pristine" > "$FX/apps/majordomus-cli/src/commands/env.rs"
expect_exit 10 gate
expect_grep 'does not read session.ensure_server_on_start'
restore apps/majordomus-cli/src/commands/env.rs

sed 's/MAJORDOMUS_RUNTIME/MAJORDOMUS_SOMETHING/g' "$T/pristine" > "$FX/apps/majordomus-cli/src/commands/env.rs"
expect_exit 10 gate
expect_grep 'does not honour MAJORDOMUS_RUNTIME'
restore apps/majordomus-cli/src/commands/env.rs

# a second implementation of the convergence, which is two answers to 'is a server already
# serving this checkout' at the one moment where disagreeing means two servers
sed 's/serve::converge/my_own_converge/g' "$T/pristine" > "$FX/apps/majordomus-cli/src/commands/env.rs"
expect_exit 10 gate
expect_grep 'does not call commands::serve::converge'
restore apps/majordomus-cli/src/commands/env.rs

printf 'fn build() { Command::new("cargo").arg("build"); }\n' >> "$FX/apps/majordomus-cli/src/commands/env.rs"
expect_exit 10 gate
expect_grep 'builds; nothing on entry may build'
restore apps/majordomus-cli/src/commands/env.rs

# ------------------------------------------------- 2. the switch and the budgets are declared
# A key in the policy the schema does not know is refused at run time, so the policy stops
# parsing for everybody: the four sites are declared together or not at all.
save share/schemas/majordomus/policy/policy.v1.schema.json
sed 's/ensure_server_on_start/renamed_key/g' "$T/pristine" > "$FX/share/schemas/majordomus/policy/policy.v1.schema.json"
expect_exit 10 gate
expect_grep 'the policy schema .* does not declare ensure_server_on_start'
restore share/schemas/majordomus/policy/policy.v1.schema.json

# the budgets are keys of the policy like any other, and joined the switch with ADR 0043: a
# budget that lives in one place only is a budget nothing can be held to
save share/schemas/majordomus/policy/policy.v1.schema.json
sed 's/enter_cold_ms/renamed_budget/g' "$T/pristine" > "$FX/share/schemas/majordomus/policy/policy.v1.schema.json"
expect_exit 10 gate
expect_grep 'the policy schema .* does not declare enter_cold_ms'
restore share/schemas/majordomus/policy/policy.v1.schema.json

save .ai/repo/policy.yaml
sed 's/^    enter_ms:.*$//' "$T/pristine" > "$FX/.ai/repo/policy.yaml"
expect_exit 10 gate
expect_grep "this repository's policy .* does not declare enter_ms"
restore .ai/repo/policy.yaml

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
# The lease half — that nothing but the typed reader opens the file — belongs to
# project.the-lease-is-read-once and scripts/ci/lease-reader-check, and is deliberately not
# checked twice; what is this rule's is the other way of forming a second opinion, which
# opens no file and so is invisible to that gate.
save lib/derive.sh
printf 'mj_derive_server() { curl -fsS "$url/api/v1/server"; }\n' >> "$FX/lib/derive.sh"
expect_exit 10 gate
expect_grep 'second opinion'
restore lib/derive.sh

# ---------------------------------------------------------------- 5. a bounded life
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

# the descriptors the caller held: a server that keeps them holds a direnv shell for the
# whole idle life, which cost 481 seconds on 2026-09-11 and which no test could see
sed 's/cmd\.pre_exec(/cmd.inherit_everything(/' "$T/pristine" > "$FX/apps/majordomus-cli/src/commands/serve.rs"
expect_exit 10 gate
expect_grep 'without closing the caller'
restore apps/majordomus-cli/src/commands/serve.rs

# ---------------------------------------------------------------- and clean again
# Every restore put back what it took: if one did not, this is where it is seen.
expect_exit 0 gate
expect_no_grep '^FAIL'

# an unusable invocation is an error, never a pass
expect_exit 2 env MJ_ROOT="$FX" "$GATE" --nonsense

echo "    the entry gate refuses each thing the rule forbids, and accepts the tree that does not"
