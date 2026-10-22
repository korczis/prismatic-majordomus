# majordomus-exclusive: the gate plants a canary under .ai/local/ and removes it; a case running beside it must not see it
# Nothing from the local half of the AI layer reaches the public site.
#
# `.ai/` is two halves. `.ai/repo/` is the tracked context a repository publishes about itself;
# `.ai/local/` is this checkout's own working state — the prompts a person actually typed, the
# session contexts a worker was handed, the ledger, the open questions, the handovers. The
# manifest marks it `tracked: false` and git ignores it. The only way any of it can become
# public is if the site generator reads it, and `scripts/generate-site-data` excludes it at
# exactly one line:
#
#   case "$p" in "$LOCAL"/*) continue ;; ...
#
# That line had no test. Delete it and every gate in this repository stays green; the next
# build starts resolving paths under a directory holding three thousand raw prompts, and the
# thing that notices is a person reading the published site. The plan item that asked for the
# gate (site/content/plan/I1303.md, "prove with a gate that nothing published ever reaches it")
# had been open since the workspace kind was designed.
#
# The gate is `scripts/ci/local-never-published`. What is held here is each of its four
# questions against a fixture that answers it wrongly, because a gate is only worth what it
# refuses. In particular the first one — plant a file under the local root and ask the
# generator for its input fingerprint again — is behavioural rather than textual: it does not
# care where the exclusion is written or what it is called, so it survives the refactor that
# would delete the line above.
. "$ROOT/test/lib.sh"

GATE="$ROOT/scripts/ci/local-never-published"
[ -x "$GATE" ] || { echo "    scripts/ci/local-never-published is missing or not executable"; exit 1; }

# A checkout with a layer, a local store, a published tree, and a stand-in generator whose
# treatment of the local root the case chooses. The real generator is forty seconds and needs
# the whole repository; what is under test is the gate's question, and the question is "does
# planting a file under the local root move the number this command prints".
fixture() {
  local dir="$1" mode="$2"
  rm -rf "$dir"; mkdir -p "$dir/scripts/ci" "$dir/.ai/local/prompts" "$dir/site/public" "$dir/site/data/generated"
  cp "$GATE" "$dir/scripts/ci/local-never-published"
  cp -R "$ROOT/lib" "$dir/lib"
  cp -R "$ROOT/bin" "$dir/bin"
  cp -R "$ROOT/share" "$dir/share"
  mkdir -p "$dir/.ai"
  cp "$ROOT/.ai/manifest.yaml" "$ROOT/.ai/README.md" "$dir/.ai/"
  printf 'the prompt a person actually typed, which nobody else is owed\n' \
    > "$dir/.ai/local/prompts/20260911-something.md"
  printf '<html><body>a published page</body></html>\n' > "$dir/site/public/index.html"
  printf '{"schema":1}\n' > "$dir/site/data/generated/thing.json"
  # the stand-in generator: `--fingerprint` over the tree, either skipping the local half the
  # way the real one does, or not
  case "$mode" in
    excludes) cat > "$dir/scripts/generate-site-data" <<'GEN'
#!/usr/bin/env bash
[ "${1:-}" = --fingerprint ] || exit 2
find . -type f -not -path './.git/*' -not -path './.ai/local/*' | LC_ALL=C sort | shasum -a 256 | cut -d' ' -f1
GEN
      ;;
    includes) cat > "$dir/scripts/generate-site-data" <<'GEN'
#!/usr/bin/env bash
[ "${1:-}" = --fingerprint ] || exit 2
find . -type f -not -path './.git/*' | LC_ALL=C sort | shasum -a 256 | cut -d' ' -f1
GEN
      ;;
  esac
  chmod +x "$dir/scripts/generate-site-data"
  (
    cd "$dir" || exit 1
    git init -q .
    git config user.email t@e; git config user.name t
    printf '.ai/local/\n' > .gitignore
    git add -A >/dev/null 2>&1
    git commit -qm "a checkout" >/dev/null 2>&1
  )
}

# Hermetic, for the reason case 180 states: this repository's .envrc exports MAJORDOMUS_ROOT
# and MAJORDOMUS_SHARE at the checkout a person is sitting in, and a fixture that inherits them
# measures that checkout instead of itself.
gate() {
  local dir="$1" out="$2"; shift 2
  ( cd "$dir" && env -u MAJORDOMUS_ROOT -u MAJORDOMUS_SHARE \
      ./scripts/ci/local-never-published "$@" > "$out" 2>&1 )
}

# --- a generator that skips the local half is accepted, and says how it knows
fixture "$T/clean" excludes
if gate "$T/clean" "$T/clean.out"; then rc=0; else rc=$?; fi
[ "$rc" = 0 ] || { echo "    the gate refused a site the local store is not an input of (exit $rc):"; cat "$T/clean.out"; exit 1; }
grep -q "is not an input of the site" "$T/clean.out" || {
  echo "    the gate did not say how it knows the local store is not an input:"; cat "$T/clean.out"; exit 1; }

# --- and the whole point: a generator that reads the local half is refused
#
# One line deleted from scripts/generate-site-data is the whole distance between these two
# fixtures. Nothing textual is asserted: the fingerprint moved, so the store is an input, so
# everything in it is one build from being public.
fixture "$T/leak" includes
if gate "$T/leak" "$T/leak.out"; then rc=0; else rc=$?; fi
[ "$rc" = 10 ] || { echo "    the gate accepted a site the local store IS an input of (exit $rc):"; cat "$T/leak.out"; exit 1; }
grep -q "IS an input of the site" "$T/leak.out" || {
  echo "    the refusal does not say the local store is an input:"; cat "$T/leak.out"; exit 1; }
grep -q "prompts" "$T/leak.out" || {
  echo "    the refusal does not say what is at stake:"; cat "$T/leak.out"; exit 1; }

# --- a published file that IS a local record, byte for byte
#
# The content question, not the path question. A prompt republished as /notes/whatever.html is
# the same leak under a different name, so the comparison is over hashes and survives a rename.
fixture "$T/copied" excludes
cp "$T/copied/.ai/local/prompts/20260911-something.md" "$T/copied/site/public/notes.html"
if gate "$T/copied" "$T/copied.out"; then rc=0; else rc=$?; fi
[ "$rc" = 10 ] || { echo "    the gate accepted a published copy of a local record (exit $rc):"; cat "$T/copied.out"; exit 1; }
grep -q "byte-identical" "$T/copied.out" || {
  echo "    the refusal does not say what was found:"; cat "$T/copied.out"; exit 1; }
grep -q "notes.html" "$T/copied.out" || {
  echo "    the refusal does not name the published file:"; cat "$T/copied.out"; exit 1; }

# --- a published path under the local root
fixture "$T/path" excludes
mkdir -p "$T/path/site/public/.ai/local/prompts"
printf 'anything\n' > "$T/path/site/public/.ai/local/prompts/leaked.html"
if gate "$T/path" "$T/path.out"; then rc=0; else rc=$?; fi
[ "$rc" = 10 ] || { echo "    the gate accepted a published path under the local root (exit $rc):"; cat "$T/path.out"; exit 1; }
grep -q "path(s) under the local half" "$T/path.out" || {
  echo "    the refusal does not say a local path is published:"; cat "$T/path.out"; exit 1; }

# --- tracked local state breaks the gate's own argument, so it is a refusal
#
# The argument that the fingerprint question is sufficient rests on the published bytes being a
# function of tracked files. Tracked local state falsifies that premise silently, so the gate
# states its premise rather than assuming it.
fixture "$T/tracked" excludes
( cd "$T/tracked" && git add -f .ai/local/prompts/20260911-something.md >/dev/null 2>&1 \
   && git commit -qm "a prompt that should never have been tracked" >/dev/null 2>&1 )
if gate "$T/tracked" "$T/tracked.out"; then rc=0; else rc=$?; fi
[ "$rc" = 10 ] || { echo "    the gate accepted tracked local state (exit $rc):"; cat "$T/tracked.out"; exit 1; }
grep -q "tracked: false" "$T/tracked.out" || {
  echo "    the refusal does not say why tracked local state is wrong:"; cat "$T/tracked.out"; exit 1; }

# --- an empty local store is a check that could not be made, never a check that passed
#
# On a fresh clone and on every CI runner the local half is empty. The content comparison
# therefore has nothing to compare, and reporting that as `ok` is the defect class this
# repository keeps re-finding. It must read as unmeasured.
fixture "$T/empty" excludes
rm -rf "$T/empty/.ai/local"
if gate "$T/empty" "$T/empty.out"; then rc=0; else rc=$?; fi
[ "$rc" = 0 ] || { echo "    the gate failed on an empty local store (exit $rc):"; cat "$T/empty.out"; exit 1; }
grep -q "^note  .*is empty in this checkout" "$T/empty.out" || {
  echo "    an absent comparison was not reported as unmeasured:"; cat "$T/empty.out"; exit 1; }

# --- and this repository's own tree passes it, probe included
#
# The whole gate, against the real checkout, which is the only place the first question can be
# asked of the real generator. It plants one file under the local root and removes it, which is
# why this case declares itself exclusive: run.sh then runs it alone, and no case beside it can
# see the canary. `scripts/pages check` runs the other three questions on the publication path,
# where the bytes are made; the fingerprint question is a property of the tree and is held here.
if ( cd "$ROOT" && env -u MAJORDOMUS_ROOT -u MAJORDOMUS_SHARE \
      ./scripts/ci/local-never-published > "$T/real.out" 2>&1 ); then rc=0; else rc=$?; fi
[ "$rc" = 0 ] || { echo "    this repository's own published trees do not pass the gate (exit $rc):"; cat "$T/real.out"; exit 1; }
grep -q "is not an input of the site" "$T/real.out" || {
  echo "    the fingerprint question was not answered against the real generator:"; cat "$T/real.out"; exit 1; }
[ -z "$(find "$ROOT/.ai/local" -name '.publication-canary-*' 2>/dev/null)" ] || {
  echo "    the gate left its canary behind under .ai/local/"; exit 1; }

# --- the publication path runs it
#
# A gate nothing calls is a gate that does not exist. `scripts/pages check` is what the Pages
# workflow runs over the built output before it pushes gh-pages; the check that those bytes are
# not somebody's prompts belongs where the bytes are made.
grep -q 'local-never-published' "$ROOT/scripts/pages" || {
  echo "    scripts/pages check does not run the gate; nothing on the publication path does"; exit 1; }
