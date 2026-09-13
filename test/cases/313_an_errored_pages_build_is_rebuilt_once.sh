# majordomus-covers: none
# The fourth stage of publishing, and the only one that never recovers by itself.
#
# Publishing is four stages: the build and its checks, the deploy run, the push to gh-pages,
# and GitHub's own "pages build and deployment" of that branch. The first three are this
# repository's and each of them self-heals — a failed build is red, a cancelled run is followed
# by a newer run of a newer commit, a push is idempotent and last-writer-wins. The fourth is
# GitHub's, and an errored build there is TERMINAL: GitHub does not retry it, `gh-pages` carries
# the right tree, and the public site serves an older commit until a person notices.
#
# WHAT WAS MEASURED, 2026-09-12, `gh api repos/korczis/prismatic-majordomus/pages/builds`:
# thirty-two builds that day, twenty-two `built`, nine `errored`, one still building. Every one
# of the nine errored builds carried `duration: 0` and ended at the exact second the next build
# began — they were superseded by a newer push to gh-pages, not broken by content. Five ran in
# a row between 20:07 and 21:08 and the site stood two commits back for the length of it.
#
# Until this case, nothing in this repository had ever sent `POST /repos/{owner}/{repo}/pages/
# builds` — the one call that asks GitHub to build the branch again. `scripts/ci/pages-check`
# reported the failure and the publication run went red for it, and a red tick does not publish
# a site.
#
# THE THING THIS CASE MOSTLY EXISTS TO FORBID. A rebuild requested while a build is running
# KILLS that build. The 20:55:01Z row above is exactly that: a rebuild sent by hand during a
# build of gh-pages 14e7f1050, and the build of 14e7f1050 errored 88 seconds in. So the
# proposition is not "an errored build is retried" but the narrower one that is actually safe:
#
#     a rebuild is requested ONCE, and only when GitHub's newest build is of this very
#     commit, is `errored`, and nothing is `building` or `queued`.
#
# Six of the nine sections below are the states in which nothing may be sent. A retry that
# fires in any of them is worse than no retry at all, because it manufactures the failure it
# was written to repair — and the count of POSTs is asserted, not the exit code alone, because
# an unconditional retry and a guarded one can agree about every exit and differ about the
# only thing that matters.
#
# Nothing here reaches GitHub: `gh` is a stub on PATH which reads a builds document this case
# writes, records every POST it is sent, and — as the real endpoint does — starts a new build
# when it receives one.
. "$ROOT/test/lib.sh"

PAGES="$ROOT/scripts/pages"
GATE="$ROOT/scripts/ci/pages-check"
[ -x "$PAGES" ] || { echo "    scripts/pages is missing or not executable"; exit 1; }
[ -x "$GATE" ] || { echo "    scripts/ci/pages-check is missing or not executable"; exit 1; }
command -v jq >/dev/null 2>&1 || { echo "    skip: jq is required"; exit 0; }

# A commit that is not the head of the fixture's gh-pages. The head's own sha is whatever git
# made of the deploy commit and is read back from the fixture rather than chosen here.
OTHER_SHA=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb

# A repository whose gh-pages head is a deploy commit of its own and whose model the commands read.
# `scripts/pages` resolves its layer through lib/common.sh, so the manifest, bin and share have
# to be there before it will say anything at all; a fixture missing them reports "not measured"
# for reasons that have nothing to do with what is being asserted.
fixture() {
  local dir="$1"
  rm -rf "$dir"; mkdir -p "$dir/scripts/ci" "$dir/.ai/repo/ci" "$dir/site"
  cp "$PAGES" "$dir/scripts/pages"
  cp "$GATE" "$dir/scripts/ci/pages-check"
  cp -R "$ROOT/lib" "$dir/lib"
  cp -R "$ROOT/bin" "$dir/bin"
  cp -R "$ROOT/share" "$dir/share"
  cp "$ROOT/.ai/manifest.yaml" "$ROOT/.ai/README.md" "$dir/.ai/"
  cp "$ROOT/.ai/repo/ci/pages.yaml" "$dir/.ai/repo/ci/pages.yaml"
  cp "$ROOT/.ai/repo/ci/gates.yaml" "$dir/.ai/repo/ci/gates.yaml"
  # nothing listens there, so the live-site half refuses and stage 4 still runs and reports —
  # which is the half this case reads
  printf 'base_url = "http://127.0.0.1:1"\n' > "$dir/site/config.toml"
  (
    cd "$dir" || exit 1
    git init -q .
    git config user.email t@e; git config user.name t
    git add -A >/dev/null 2>&1
    git commit -qm first
    git branch -M master
    PUB="$(git rev-parse HEAD)"
    empty="$(git hash-object -t tree /dev/null)"
    gp="$(git commit-tree "$empty" -m "deploy: site from $PUB (registry deadbeefcafe), 800 routes")"
    # the gh-pages head must be the sha the stub answers about, and a commit's sha is not
    # chosen — so the stub is told what git produced rather than the other way round
    git update-ref refs/heads/gh-pages "$gp"
    git remote add origin "$dir"
    git fetch -q origin 'refs/heads/*:refs/remotes/origin/*'
    printf '%s\n' "$gp" > .ghpages
  )
}

# GitHub's Pages build API as a stub over a document this case writes. The --jq filter is
# handed to the real jq rather than re-implemented, so what is exercised is the filter the
# command actually sends. A POST is recorded and then does what the real endpoint does: it
# starts a build, whose outcome this case decides with $DIR/post-status.
stub_gh() {
  local dir="$1"
  mkdir -p "$dir"
  cat > "$dir/gh" <<STUB
#!/usr/bin/env bash
STATE="$dir"
case "\$*" in
  *"-X POST"*pages/builds*)
    echo "POST" >> "\$STATE/posts"
    next="\$(cat "\$STATE/post-status" 2>/dev/null || echo built)"
    jq --arg s "\$next" --arg c "\$(cat "\$STATE/post-commit")" \\
       '[{status: \$s, commit: \$c, error: {message: "Page build failed."}}] + .' \\
       "\$STATE/builds.json" > "\$STATE/builds.next" && mv "\$STATE/builds.next" "\$STATE/builds.json"
    exit 0 ;;
  *pages/builds*)
    filter=""
    prev=""
    for a in "\$@"; do [ "\$prev" = --jq ] && filter="\$a"; prev="\$a"; done
    [ -n "\$filter" ] || filter='.'
    jq -r "\$filter" "\$STATE/builds.json"
    exit 0 ;;
esac
exit 1
STUB
  chmod +x "$dir/gh"
}

# The builds document, newest first: "status:commit" pairs, in order.
builds() {
  local out="$BIN/builds.json" first=1
  printf '[' > "$out"
  for pair in "$@"; do
    [ "$first" = 1 ] || printf ',' >> "$out"
    first=0
    printf '{"status":"%s","commit":"%s","error":{"message":"Page build failed."}}' \
      "${pair%%:*}" "${pair##*:}" >> "$out"
  done
  printf ']\n' >> "$out"
  : > "$BIN/posts"
}

posts() { wc -l < "$BIN/posts" | tr -d ' '; }

# Every invocation runs inside the fixture with the stub ahead of anything real on PATH, and
# with this repository's own exported roots unset: inside a fixture they make the tool resolve
# another tree's layer, and a fixture is hermetic or it is not a fixture.
rebuild() {
  local out="$1"; shift
  ( cd "$REPO" \
    && PATH="$BIN:$PATH" env -u MAJORDOMUS_ROOT -u MAJORDOMUS_SHARE \
       ./scripts/pages rebuild --branch gh-pages "$@" > "$out" 2>&1 )
}
check() {
  local out="$1"; shift
  ( cd "$REPO" \
    && PATH="$BIN:$PATH" env -u MAJORDOMUS_ROOT -u MAJORDOMUS_SHARE \
       ./scripts/ci/pages-check --wait 0 "$@" > "$out" 2>&1 )
}

REPO="$T/repo"; BIN="$T/bin"
fixture "$REPO"
GP="$(cat "$REPO/.ghpages")"
stub_gh "$BIN"
printf '%s\n' "$GP" > "$BIN/post-commit"

# ---------------------------------------------------------------- 1. the state it recovers
#
# The whole reason this exists: GitHub's newest build is of the head of gh-pages, it errored,
# and nothing is building. Nobody will ever build it again unless something asks.
builds "errored:$GP"
printf 'built\n' > "$BIN/post-status"
if rebuild "$T/1.out"; then rc=0; else rc=$?; fi
[ "$rc" = 0 ] || { echo "    a terminal errored build was not recovered (exit $rc):"; cat "$T/1.out"; exit 1; }
[ "$(posts)" = 1 ] || { echo "    expected exactly one rebuild request, got $(posts):"; cat "$T/1.out"; exit 1; }
grep -q "requesting one rebuild" "$T/1.out" || {
  echo "    the recovery does not say what it did:"; cat "$T/1.out"; exit 1; }
grep -q "is built" "$T/1.out" || {
  echo "    the recovery does not report the verdict of the build it asked for:"; cat "$T/1.out"; exit 1; }

# ---------------------------------------------------------------- 2. A BUILD IS IN FLIGHT
#
# The section this case is mostly for. A POST here kills the build that is running — gh-pages
# 14e7f1050, 2026-09-12T20:55:01Z, 88 seconds in — so the errored build below must not be
# treated as a thing to recover while GitHub is already building the same commit.
builds "building:$GP" "errored:$GP"
if rebuild "$T/2.out"; then rc=0; else rc=$?; fi
# the count is asserted before the exit, because it is the claim that matters: a command can
# get the exit right and still have killed a build on its way there
[ "$(posts)" = 0 ] || {
  echo "    A REBUILD WAS REQUESTED WHILE A BUILD WAS RUNNING — that kills the running build,"
  echo "    which is how gh-pages 14e7f1050 errored at 2026-09-12T20:55:01Z:"
  cat "$T/2.out"; exit 1; }
[ "$rc" = 12 ] || { echo "    a rebuild was judged while a build was in flight (exit $rc):"; cat "$T/2.out"; exit 1; }
grep -q "already building" "$T/2.out" || {
  echo "    the refusal to act does not say why:"; cat "$T/2.out"; exit 1; }

# and `queued` is the same state under another name
builds "queued:$GP" "errored:$GP"
if rebuild "$T/2b.out"; then rc=0; else rc=$?; fi
[ "$rc" = 12 ] || { echo "    a queued build was not treated as a build in flight (exit $rc):"; cat "$T/2b.out"; exit 1; }
[ "$(posts)" = 0 ] || { echo "    a rebuild was requested while a build was queued:"; cat "$T/2b.out"; exit 1; }

# ---------------------------------------------------------------- 3. the error was superseded
#
# An errored build with a newer build after it was killed by that newer build, and the newer
# one is of a newer tree. Recovering the old one would publish a tree that has been replaced.
builds "errored:$OTHER_SHA" "errored:$GP"
if rebuild "$T/3.out"; then rc=0; else rc=$?; fi
[ "$rc" = 12 ] || { echo "    a superseded error was treated as terminal (exit $rc):"; cat "$T/3.out"; exit 1; }
[ "$(posts)" = 0 ] || { echo "    a rebuild was requested for a build something newer had replaced:"; cat "$T/3.out"; exit 1; }
grep -q "newest build is of ${OTHER_SHA:0:12}" "$T/3.out" || {
  echo "    the refusal does not name the build that is actually newest:"; cat "$T/3.out"; exit 1; }

# ---------------------------------------------------------------- 4. nothing is wrong
#
# The build succeeded. A retry here costs a build GitHub did not need to do and, if one has
# started since, kills it.
builds "built:$GP"
if rebuild "$T/4.out"; then rc=0; else rc=$?; fi
[ "$rc" = 0 ] || { echo "    a healthy publication was reported as a problem (exit $rc):"; cat "$T/4.out"; exit 1; }
[ "$(posts)" = 0 ] || { echo "    a rebuild was requested for a build that had succeeded:"; cat "$T/4.out"; exit 1; }
grep -q "nothing to recover" "$T/4.out" || {
  echo "    the command does not say why it did nothing:"; cat "$T/4.out"; exit 1; }

# ---------------------------------------------------------------- 5. nothing can be read
#
# No token, no `pages: read`, no API. A command that cannot see the state must not act on a
# guess about it, and must not report that as success either.
builds
if rebuild "$T/5.out"; then rc=0; else rc=$?; fi
[ "$rc" = 12 ] || { echo "    an unreadable API was not reported as unreadable (exit $rc):"; cat "$T/5.out"; exit 1; }
[ "$(posts)" = 0 ] || { echo "    a rebuild was requested without reading the state first:"; cat "$T/5.out"; exit 1; }

# ---------------------------------------------------------------- 6. the rebuild errors too
#
# One attempt. The site is still stale, this is now definite rather than weather, and the
# second POST that a loop would send is exactly what must not happen.
builds "errored:$GP"
printf 'errored\n' > "$BIN/post-status"
if rebuild "$T/6.out"; then rc=0; else rc=$?; fi
[ "$rc" = 10 ] || { echo "    a rebuild that errored again was not a refusal (exit $rc):"; cat "$T/6.out"; exit 1; }
[ "$(posts)" = 1 ] || {
  echo "    the retry is not bounded at one attempt — $(posts) rebuild requests were sent:"
  cat "$T/6.out"; exit 1; }
grep -q "ERRORED" "$T/6.out" || {
  echo "    the refusal does not carry GitHub's own verdict:"; cat "$T/6.out"; exit 1; }

# ---------------------------------------------------------------- 7. and one attempt in the code
#
# The bound above is a property of this program and not of the number of times a caller
# happened to run it: there is one POST in the source and it is not inside a loop. A counter
# that says "1" is a bound somebody can raise; no loop is a bound nobody can.
[ "$(grep -c 'gh api -X POST "repos/\$slug/pages/builds"' "$PAGES")" = 1 ] || {
  echo "    scripts/pages sends the rebuild in more than one place, or not at all"; exit 1; }
# the body of cmd_rebuild, from its opening line to the closing brace in column one
awk '/^cmd_rebuild\(\) \{/ { f = 1 } f { print } f && /^\}$/ { exit }' "$PAGES" > "$T/cmd_rebuild.sh"
[ -s "$T/cmd_rebuild.sh" ] || { echo "    cmd_rebuild could not be read out of scripts/pages"; exit 1; }
grep -q 'gh api -X POST' "$T/cmd_rebuild.sh" || {
  echo "    the rebuild request is not inside cmd_rebuild, so what bounds it is not this function"; exit 1; }
# `case ... esac` arms and the option parser's own `while` are not loops over the request: the
# body is read for any loop keyword that is not the argument loop, which ends before the POST.
sed '1,/^  esac; done$/d' "$T/cmd_rebuild.sh" | grep -nE '^[[:space:]]*(while|until|for) ' > "$T/loops" || true
[ ! -s "$T/loops" ] || {
  echo "    the rebuild request sits inside a loop, so 'once' is a hope rather than a bound:"
  cat "$T/loops"; exit 1; }

# ---------------------------------------------------------------- 8. the gate names the remedy
#
# `pages-check` refuses an errored build already; what it could not do until now was tell a
# reader what to do about it, because there was nothing to do. The remedy is a command now and
# the refusal must name it, or the gate is a notification rather than an instruction.
builds "errored:$GP"
if check "$T/8.out"; then rc=0; else rc=$?; fi
[ "$rc" = 10 ] || { echo "    the gate accepted a branch GitHub could not build (exit $rc):"; cat "$T/8.out"; exit 1; }
grep -q "scripts/pages rebuild" "$T/8.out" || {
  echo "    the refusal does not name the command that repairs it:"; cat "$T/8.out"; exit 1; }
grep -q "never retries an errored" "$T/8.out" || {
  echo "    the refusal does not say that the error is terminal, which is why it needs acting on:"
  cat "$T/8.out"; exit 1; }

# ---------------------------------------------------------------- 9. and it stops contradicting itself
#
# Run against this repository on 2026-09-12 the gate printed, verbatim:
#
#   note  GitHub's newest Pages build is of 14e7f1050b6a (building), not the head of gh-pages
#         14e7f1050b6a
#
# The two shas in that sentence are the same sha. The reading is right — a build in flight is
# weather and stays a note — and the words are unactionable, which for the one line a person
# reads during an outage is most of the defect.
builds "building:$GP"
if check "$T/9.out"; then rc=0; else rc=$?; fi
grep -qE "^note  .*Pages build of ${GP:0:12} — the head of gh-pages — is building" "$T/9.out" || {
  echo "    a build in flight is not reported as the head's own:"; cat "$T/9.out"; exit 1; }
grep -q "not the head of gh-pages ${GP:0:12}" "$T/9.out" && {
  echo "    the gate still says a commit is not itself:"; cat "$T/9.out"; exit 1; }

# a build of some OTHER commit is the case that sentence was written for, and it still says so
builds "built:$OTHER_SHA"
if check "$T/9b.out"; then rc=0; else rc=$?; fi
grep -q "not the head of gh-pages ${GP:0:12}" "$T/9b.out" || {
  echo "    a build of another commit is no longer distinguished from the head's own:"; cat "$T/9b.out"; exit 1; }

# ---------------------------------------------------------------- 10. and the run that publishes sends it
#
# The gate is what notices on the next validation of master. The publication run is what is
# standing there holding the token at the moment the build errors, and it is the only thing in
# a position to repair the site rather than report it. Once, and with the permission the call
# needs — `read` cannot send a POST.
WF="$ROOT/.github/workflows/pages.yml"
grep -q 'scripts/pages rebuild' "$WF" || {
  echo "    the publication run reads GitHub's errored build and does nothing about it;"
  echo "    GitHub never retries it, so the site stays stale until a person notices"; exit 1; }
# once, and counted over what the run executes rather than over what it explains: a comment
# that names the command is documentation, and grepping the whole file made the reasoning
# above this step read as a second call. Zero and two are different defects and are named
# differently — a case whose message fits both tells a reader neither.
# `|| true`: grep -c exits 1 when the count is zero, and a case runs under set -e, so the
# assignment itself would end the case with no message at all — which is precisely the
# defect being asserted, reported as nothing. It happened here while this case was written.
calls="$(grep -vE '^[[:space:]]*#' "$WF" | grep -c 'scripts/pages rebuild' || true)"
[ "$calls" != 0 ] || {
  echo "    the publication run explains the rebuild and never runs it; the site stays stale"; exit 1; }
[ "$calls" = 1 ] || {
  echo "    the publication run asks for $calls rebuilds; one attempt is the bound"; exit 1; }
grep -q 'pages: write' "$WF" || {
  echo "    the publication run asks for a rebuild with a permission that cannot send one"; exit 1; }

echo "    an errored Pages build is rebuilt once, and never while a build is running"
