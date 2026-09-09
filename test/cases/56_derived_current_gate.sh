# majordomus-covers: none
# majordomus-negative: doctor
# The gate that refuses a commit whose derived data is behind its canonical inputs, and the
# doctrine that refuses a repository which unwires the gate.
#
# This case runs in the tool's own checkout rather than a disposable repository: the thing
# under test is this repository's committed derived data and the hook that guards it, and
# neither exists anywhere else. It writes nothing — every mutation below is made on a copy.
. "$ROOT/test/lib.sh"

# ---------------------------------------------------------------- the gate answers
# The tree under test is HEAD, not the working copy: a person mid-change has inputs moved and
# nothing regenerated, which is the state the gate exists to catch and not a fault of the
# gate. What must hold is that the tree somebody committed is current — if this fails, a
# stale commit got in, which is the failure this whole case is about.
W="$T/tree"
mkdir -p "$W"
(cd "$ROOT" && git archive HEAD) | (cd "$W" && tar xf -) || {
  echo "    could not export HEAD"; exit 1; }
# The export is a repository, not just a tree, for the same reason the second half's is
# below: the gate asks both halves, and the registry half discovers its index with
# `git ls-files`. A tree with no `.git` of its own resolves to the runner's fixture repository
# around it, indexes nothing, and reads every artifact as stale. Locally that half is skipped
# for want of an executable and the case passes; in CI, where MAJORDOMUS_BIN is set, it ran
# and failed on a tree that is current.
(cd "$W" && git init -q . && git config user.email t@example.com && git config user.name t \
  && git add -A && git commit -qm export) || { echo "    could not make the export a repository"; exit 1; }

out="$(cd "$W" && scripts/pages current 2>&1)" || {
  echo "    the committed tree is not current; run scripts/derive and commit the result"
  printf '    | %s\n' "$out"; exit 1; }
case "$out" in
  *"is current for this tree"*) ;;
  *) echo "    pages current did not report the tree as current: $out"; exit 1 ;;
esac

# it is fast enough to sit in front of every commit — the whole point of the fingerprint
t0=$(date +%s); (cd "$W" && scripts/pages current >/dev/null 2>&1); t1=$(date +%s)
[ "$((t1 - t0))" -le 20 ] || { echo "    the gate took $((t1 - t0))s; it runs on every commit and must not"; exit 1; }

# ---------------------------------------------------------------- a moved input is refused
# An input is moved in that same export and it is asked again. A canonical input is one the
# generator names; the list comes from the generator itself rather than from a second list
# here, which is the mistake this repository has made before.
input="$(cd "$W" && scripts/generate-site-data --inputs | grep '^docs/' | head -n 1)"
[ -n "$input" ] || { echo "    the generator names no canonical input under docs/"; exit 1; }
printf '\nA line that moves the canonical inputs.\n' >> "$W/$input"

rc=0; out="$(cd "$W" && scripts/pages current 2>&1)" || rc=$?
[ "$rc" = 10 ] || { echo "    a moved input did not make the gate exit 10 (got $rc)"; printf '    | %s\n' "$out"; exit 1; }
case "$out" in
  *"is stale"*"input hash"*) ;;
  *) echo "    the refusal does not name the hashes: $out"; exit 1 ;;
esac
# a refusal that does not say what to run is a dead end, and what repairs it is the script
# that runs both generators — naming only one of them is how a tree gets half regenerated
case "$out" in
  *scripts/derive*) ;;
  *) echo "    the refusal does not name the command that repairs it: $out"; exit 1 ;;
esac

# ---------------------------------------------------------------- and the other half
# The derived data has two owners. `scripts/generate-site-data` writes site/data/generated,
# which the fingerprint above covers; `majordomus generate` writes the registry and
# docs/generated/artifacts.*, which it does not, because the registry records every source
# file it indexed and a source file is not a site input. A commit that moved a source file
# and regenerated only the first half passed this gate and failed CI afterwards, for
# somebody else to read as staleness they did not cause. Both halves are asked here.
RB="$(rust_bin)" || rust_bin_exit $?
# this half needs a git repository, not just a tree: the index is discovered with
# `git ls-files`, so an export with no `.git` indexes a different set and every artifact
# reads as stale for a reason that has nothing to do with the gate
W2="$T/tree2"; mkdir -p "$W2"
(cd "$ROOT" && git archive HEAD) | (cd "$W2" && tar xf -) || { echo "    could not export HEAD"; exit 1; }
(cd "$W2" && git init -q . && git config user.email t@example.com && git config user.name t \
  && git add -A && git commit -qm export) || { echo "    could not make the export a repository"; exit 1; }

rc=0; out="$(cd "$W2" && MAJORDOMUS_BIN="$RB" scripts/pages current 2>&1)" || rc=$?
[ "$rc" = 0 ] || { echo "    the committed tree fails the registry half (got $rc)"; printf '    | %s\n' "$out"; exit 1; }
case "$out" in
  *"registry projections are in sync"*) ;;
  *) echo "    the gate did not report the registry half at all: $out"; exit 1 ;;
esac

# a source file the registry indexes, moved: the site fingerprint does not notice and the
# registry half must
printf '\n# a line that moves the registry and not the site data\n' >> "$W2/lib/context_docs.sh"
rc=0; out="$(cd "$W2" && MAJORDOMUS_BIN="$RB" scripts/pages current 2>&1)" || rc=$?
[ "$rc" = 10 ] || { echo "    a moved source file did not make the gate exit 10 (got $rc)"; printf '    | %s\n' "$out"; exit 1; }
case "$out" in
  *"is current for this tree"*) ;;
  *) echo "    the site half should still pass; this case is about the other one: $out"; exit 1 ;;
esac
case "$out" in
  *registry.json*) ;;
  *) echo "    the refusal does not name what is stale: $out"; exit 1 ;;
esac
case "$out" in
  *scripts/derive*) ;;
  *) echo "    the refusal does not name the command that repairs it: $out"; exit 1 ;;
esac

# a repository without the executable is told so rather than passed silently: a gate that
# cannot run and says nothing is the failure this whole case is about
out="$(cd "$W2" && MAJORDOMUS_BIN=/nonexistent scripts/pages current 2>&1)" || true
case "$out" in
  *"was not checked"*) ;;
  *) echo "    with no executable the gate did not say the registry half went unchecked: $out"; exit 1 ;;
esac

# ---------------------------------------------------------------- the hook carries it
grep -qE 'scripts/pages[[:space:]]+current' "$ROOT/.githooks/pre-commit" \
  || { echo "    .githooks/pre-commit does not invoke the gate"; exit 1; }
if grep -E 'scripts/pages[[:space:]]+current' "$ROOT/.githooks/pre-commit" | grep -qE '\|\|[[:space:]]*(true|exit[[:space:]]+0)'; then
  echo "    the hook swallows the gate's exit code"; exit 1
fi

# ---------------------------------------------------------------- and doctrine holds the hook
# The gate is declared in the policy, so unwiring it is a finding rather than a silence. The
# copy is used again: the hook loses the line, and doctor must go red on the wiring entry
# rather than reporting the repository as sound.
grep -q 'name: derived-current' "$ROOT/.ai/repo/policy.yaml" \
  || { echo "    the policy does not declare the gate as an enforcement entry"; exit 1; }

# The declaration and the hook are taken from the tree rather than from HEAD: they are what
# this change is about, and a case that could only judge the previous commit would pass for
# the wrong reason on the commit that introduces them. After the commit the two agree.
cp "$ROOT/.ai/repo/policy.yaml" "$W/.ai/repo/policy.yaml"
git -C "$W" init -q . 2>/dev/null || true
mkdir -p "$W/.git/hooks"
printf '#!/usr/bin/env bash\nbin/majordomus doctor\n' > "$W/.git/hooks/pre-commit"
printf '#!/usr/bin/env bash\nbin/majordomus finish --check\n' > "$W/.git/hooks/pre-push"
chmod +x "$W/.git/hooks/pre-commit" "$W/.git/hooks/pre-push"
grep -v 'scripts/pages' "$ROOT/.githooks/pre-commit" > "$W/.githooks/pre-commit"
chmod +x "$W/.githooks/pre-commit"
(cd "$W" && git config core.hooksPath .githooks)

(cd "$W" && "$ROOT/bin/majordomus" doctor > "$T/doctor.out" 2>&1) || true
grep -qE 'FAIL +wiring +derived-current' "$T/doctor.out" \
  || { echo "    doctor did not report the unwired gate"; grep -E 'wiring' "$T/doctor.out" | sed 's/^/    | /'; exit 1; }
grep -qE 'derived-current.*not invoked by' "$T/doctor.out" \
  || { echo "    the finding does not say the hook stopped invoking it"; grep -E 'derived-current' "$T/doctor.out" | sed 's/^/    | /'; exit 1; }
