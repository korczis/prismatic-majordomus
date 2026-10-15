# majordomus-covers: none
# The UI the public site holds at the UI's mount is the trunk's, and its absence, its age and
# its being cut short without saying so are failures (project.the-published-ui-is-the-trunks).
#
# Since ADR 0141 a publication holds the UI as a snapshot read at one commit, written by a job
# of its own and placed by the deploy. Until 2026-10-12 nothing asked the public site for it:
# the probes read /build.json, which does not name it, and a person's `scripts/site-deploy`
# published the site without it. Held here, with no Rust executable, no zola, no network and
# no token — the site is a directory behind test/lib.sh's own server, the remote a bare
# repository, and the snapshot a stand-in, because the producer is cases 1092 and 1093's:
#
#   1. the declaration   the publication model names the snapshot once, `scripts/pages
#                        snapshots` resolves its mount from the topology, and the pages
#                        workflow builds and uploads what that entry says
#   2. the probe         `scripts/pages verify-ui` passes a snapshot read at the commit, with
#                        and without --commit; refuses by name a stale built_from, a missing
#                        surface.json, a listed page that is absent, a record of another run,
#                        a first screen still waiting for a panel, and a record that does not
#                        say what ended its reading; states what a cut
#                        short snapshot left out and what --max-pages did not ask; and is 12,
#                        never a pass, for a site that does not answer
#   3. the gate          `scripts/ci/pages-check` asks it of the published commit: ok for the
#                        UI read there, exit 10 for another commit's and for none at all
#   4. the deploy        `scripts/site-deploy` refuses a site without the snapshot and one
#                        read at another commit, and publishes the one read at HEAD
#   5. wiring            the workflow runs the probe after a deploy, blocking, and the gate's
#                        summary and the rule name it
. "$ROOT/test/lib.sh"
command -v jq >/dev/null 2>&1 || skip "jq is not installed"
command -v curl >/dev/null 2>&1 || skip "curl is not installed"

S="$(mktemp -d "${TMPDIR:-/tmp}/mj2983.XXXXXX")"; trap 'stop_http; rm -rf "$S"' EXIT
gitq() { git -c user.email=t@example.com -c user.name=t -c core.hooksPath=/dev/null "$@"; }
TAB="$(printf '\t')"
pages() { env -u MAJORDOMUS_ROOT -u MAJORDOMUS_SHARE "$ROOT/scripts/pages" "$@"; }

# ---------------------------------------------------------------- 1. the declaration
rows="$(pages snapshots)" || { echo "    scripts/pages snapshots could not read the model"; exit 1; }
row="$(printf '%s\n' "$rows" | awk -F '\t' '$1 == "ui"')"
[ -n "$row" ] || { echo "    the publication model declares no snapshot of the ui surface:"; printf '%s\n' "$rows"; exit 1; }
IFS="$TAB" read -r _ mount artifact producer marker <<EOF
$row
EOF
[ "$mount" = "$(jq -r '.surfaces[] | select(.id == "ui") | .mount' "$ROOT/docs/generated/web.json")" ] \
  || { echo "    the snapshot's mount ($mount) is not the topology's mount of the ui surface"; exit 1; }
[ -n "$artifact" ] && [ -n "$producer" ] && [ -n "$marker" ] \
  || { echo "    the entry does not name its artifact, producer and marker: $row"; exit 1; }
# the mount is written in the model nowhere: it is the topology's, or it is a second declaration
if awk '/^  snapshots:/{f=1; next} f && /^  [a-z_]+:/{f=0} f && !/^ *#/' "$ROOT/.ai/repo/ci/pages.yaml" | grep -q 'mount:'; then
  echo "    deploy.snapshots writes a mount of its own beside the topology's"; exit 1
fi
# and the workflow, the adapter, builds and uploads exactly what the entry says
WF="$ROOT/.github/workflows/pages.yml"
# (the producer may be given options — how many pages, how long — and nothing else follows it)
grep -qE "^        run: .*majordomus ${producer#majordomus }( --[a-z-]+ [0-9]+)*\$" "$WF" \
  || { echo "    no step of the pages workflow runs the producer the model names ($producer)"; exit 1; }
[ "$(grep -cE "^          path: $artifact\$" "$WF")" -ge 2 ] \
  || { echo "    the pages workflow does not upload and download the artifact the model names ($artifact)"; exit 1; }
# a model that names a surface the topology does not carry has nowhere to hold it: 12
M="$S/model"; mkdir -p "$M/scripts" "$M/.ai/repo/ci" "$M/docs/generated"
cp "$ROOT/scripts/pages" "$M/scripts/pages"; cp -R "$ROOT/lib" "$M/lib"; cp -R "$ROOT/bin" "$M/bin"; cp -R "$ROOT/share" "$M/share"
cp "$ROOT/.ai/manifest.yaml" "$ROOT/.ai/README.md" "$M/.ai/"
cp "$ROOT/.ai/repo/ci/pages.yaml" "$ROOT/.ai/repo/ci/gates.yaml" "$M/.ai/repo/ci/"
jq 'del(.surfaces[] | select(.id == "ui"))' "$ROOT/docs/generated/web.json" > "$M/docs/generated/web.json"
expect_exit 12 env -u MAJORDOMUS_ROOT -u MAJORDOMUS_SHARE "$M/scripts/pages" snapshots
expect_grep 'declares a snapshot of surface ui, and docs/generated/web.json gives it no mount'
echo "    one declaration: the model names the snapshot, the topology its mount, the workflow follows"

# ---------------------------------------------------------------- 2. the probe
A=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
B=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
W="$S/www"
site() {   # <commit the site names> <commit the snapshot was read at>: a published site as files
  rm -rf "$W"; mkdir -p "$W"
  jq -n --arg c "$1" '{schema: 1, commit: $c, dirty: false}' > "$W/build.json"
  printf '<html><body>site</body></html>\n' > "$W/index.html"
  publication_snapshots "$ROOT" "$W" "$2" || { echo "    the stand-in snapshot could not be placed"; exit 1; }
}
site "$A" "$A"
start_http "$W" || skip "no python3 and no node to serve a fixture site"
probe() { pages verify-ui --url "$HTTP_BASE" --timeout 0 "$@"; }

expect_exit 0 probe --commit "$A"
expect_grep "surface\.json .*built_from=${A:0:10}\" ok\$"
expect_grep "$marker=${A:0:10}\" ok\$"
expect_grep "serves the UI read at ${A:0:12}: 2 of 2 page\(s\) snapshot\.json lists were asked for and 0 did not answer, 0 not asked"
expect_grep 'the snapshot left out 0 page\(s\), stopped by complete'
# without --commit the commit is the one the site's own identity names
expect_exit 0 probe
expect_grep "serves the UI read at ${A:0:12}"
echo "    a snapshot read at the commit the site names: verified, with and without --commit"

# (a) the pages moved on and the UI beside them did not
site "$B" "$A"
expect_exit 10 probe
expect_grep "FAIL the published UI was read at ${A:0:10}, not at $B: .*  \[reproduce: curl -fsS $HTTP_BASE$mount/surface\.json \| jq -r \.built_from\]"
expect_exit 10 probe --commit "$B"
expect_grep "surface\.json .*built_from=${A:0:10}, want $B\" fail\$"
echo "    a stale built_from: refused by name, with the command that shows it"

# (a2) the first screen leads with a figure nothing decided
site "$A" "$A"
printf '<span class="mj-ui-kpi"><span class="mj-ui-kpi-label">Repository health</span><span class="mj-ui-kpi-value"><span class="mj-badge mj-badge--unknown">unknown</span></span></span>\n' >> "$W$mount/index.html"
expect_exit 10 probe --commit "$A"
expect_grep "FAIL the first screen of $HTTP_BASE$mount/ leads with figure\(s\) nothing decided \(Repository health\)"
expect_grep "first-screen figure\(s\) undecided: Repository health\" fail\$"
site "$A" "$A"
expect_exit 0 probe --commit "$A"
expect_grep 'first-screen figures decided" ok$'
echo "    an undecided figure on the first screen: refused by name"

# (b) the deploy that lost the mount: the site answers and holds no declaration there
site "$A" "$A"; rm "$W$mount/surface.json"
expect_exit 10 probe --commit "$A"
expect_grep "FAIL $HTTP_BASE$mount/surface\.json is absent or unreadable \(status 404\): the publication holds no UI snapshot at $mount  \[reproduce: "
rm -rf "${W:?}$mount"
expect_exit 10 probe --commit "$A"
expect_grep "surface\.json is absent or unreadable \(status 404\)"
echo "    a missing surface.json, and a missing mount: refused by name"

# (c) half a copy: the record lists a page the site does not serve
site "$A" "$A"; rm "$W$mount/one/index.html"
expect_exit 10 probe --commit "$A"
expect_grep "FAIL $HTTP_BASE$mount/one/index\.html answered 404: snapshot\.json lists a page the site does not serve  \[reproduce: curl -fsSI "
expect_grep '2 of 2 page\(s\) snapshot\.json lists were asked for and 1 did not answer.*" fail$'
echo "    a page snapshot.json lists that is absent: refused by name"

# the record of another run beside this run's declaration, and a banner that is not this run's
site "$A" "$A"
jq --arg c "${B:0:10}" '.origin.revision = $c' "$W$mount/snapshot.json" > "$S/j" && cp "$S/j" "$W$mount/snapshot.json"
expect_exit 10 probe --commit "$A"
expect_grep "snapshot\.json records a reading at ${B:0:10}, not at $A"
site "$A" "$A"; sed "s/$marker=\"[0-9a-f]*\"/$marker=\"${B:0:10}\"/" "$W$mount/index.html" > "$S/h" && cp "$S/h" "$W$mount/index.html"
expect_exit 10 probe --commit "$A"
expect_grep "does not state $marker=\"${A:0:10}\""
site "$A" "$A"; rm "$W$mount/snapshot.json"
expect_exit 10 probe --commit "$A"
expect_grep 'snapshot\.json is absent or is not a snapshot.s record \(status 404\)'
echo "    a record of another run, a banner of another commit, no record at all: refused"

# a screen published with a panel still to be fetched: no process will fetch it
site "$A" "$A"
sed 's|<body|<body><section data-panel-src="panels/drain/index.html">open the drain</section><i|' "$W$mount/index.html" > "$S/h" && cp "$S/h" "$W$mount/index.html"
# a record written before the writer put panels into pages: stated, not this publication's failure
expect_exit 0 probe --commit "$A"
expect_grep 'its record predates panels written into the page \(no \.panels\)" ok$'
# a record that says its panels are in the pages, beside a screen that still asks: refused
jq '.panels = ["/ui/panels/drain"]' "$W$mount/snapshot.json" > "$S/j" && cp "$S/j" "$W$mount/snapshot.json"
expect_exit 10 probe --commit "$A"
expect_grep "FAIL $HTTP_BASE$mount/ still carries data-panel-src: the snapshot left a panel as a request no process answers"
echo "    a first screen that still waits for a panel: refused by name when its record says panels are written in"

# cut short: saying so is a pass that states it, not saying so is a refusal
site "$A" "$A"
jq '.left_out = ["/x?id=1", "/x?id=2", "/x?id=3"] | .stopped_by = "budget"' "$W$mount/snapshot.json" > "$S/j" && cp "$S/j" "$W$mount/snapshot.json"
expect_exit 0 probe --commit "$A"
expect_grep 'the snapshot left out 3 page\(s\), stopped by budget'
jq 'del(.stopped_by)' "$W$mount/snapshot.json" > "$S/j" && cp "$S/j" "$W$mount/snapshot.json"
expect_exit 10 probe --commit "$A"
expect_grep 'FAIL .*does not say what ended the reading \(stopped_by\) or which pages it left out \(left_out\)'
echo "    cut short and saying so: verified, stating 3 left out and why; not saying so: refused"

# the bound on the probe is stated, and what it did not ask is counted
site "$A" "$A"
for n in 2 3 4 5 6; do
  mkdir -p "$W$mount/p$n"; cp "$W$mount/one/index.html" "$W$mount/p$n/index.html"
  jq --arg k "$mount/p$n" --arg f "p$n/index.html" '.pages[$k] = $f' "$W$mount/snapshot.json" > "$S/j" && cp "$S/j" "$W$mount/snapshot.json"
done
expect_exit 0 probe --commit "$A" --max-pages 3
expect_grep '3 of 7 page\(s\) snapshot\.json lists were asked for and 0 did not answer, 4 not asked \(--max-pages 3\)'
# spread over the sorted list, not its head: the last page is among those asked when it is absent
rm "$W$mount/p6/index.html"
expect_exit 10 probe --commit "$A" --max-pages 7
expect_grep "p6/index\.html answered 404"
expect_exit 2 probe --commit "$A" --max-pages 0
expect_exit 2 probe --commit abc
echo "    --max-pages bounds the probe and says what it did not ask; usage errors are 2"

# a site that does not answer was not measured: 12, never a pass and never a refusal
site "$A" "$A"; base="$HTTP_BASE"; stop_http
expect_exit 12 pages verify-ui --url "$base" --timeout 0 --commit "$A"
expect_grep 'could not be judged within 0 s; the UI was not verified'
expect_exit 12 pages verify-ui --url "$base" --timeout 0
expect_grep 'no --commit, and .*build\.json names none'
echo "    a site that does not answer: 12"

# ---------------------------------------------------------------- 3. the gate
# A repository whose gh-pages names a published commit and whose site/config.toml points at the
# directory served here, as case 180 builds it; GitHub's build API and newest release are a
# stub on PATH. The subject is one section of the gate: the rest of the site is whole.
R="$S/repo"; mkdir -p "$R/scripts/ci" "$R/.ai/repo/ci" "$R/site" "$R/docs/generated" "$S/bin"
cp "$ROOT/scripts/ci/pages-check" "$R/scripts/ci/pages-check"; cp "$ROOT/scripts/pages" "$R/scripts/pages"
cp -R "$ROOT/lib" "$R/lib"; cp -R "$ROOT/bin" "$R/bin"; cp -R "$ROOT/share" "$R/share"
cp "$ROOT/.ai/manifest.yaml" "$ROOT/.ai/README.md" "$R/.ai/"
cp "$ROOT/.ai/repo/ci/pages.yaml" "$ROOT/.ai/repo/ci/gates.yaml" "$R/.ai/repo/ci/"
cp "$ROOT/docs/generated/web.json" "$R/docs/generated/web.json"
start_http "$W" || { echo "    the fixture server would not restart"; exit 1; }
printf 'base_url = "%s"\n' "$HTTP_BASE" > "$R/site/config.toml"
git init -q -b master "$R"; gitq -C "$R" add -A >/dev/null 2>&1; gitq -C "$R" commit -qm first
PUB="$(git -C "$R" rev-parse HEAD)"
empty="$(git -C "$R" hash-object -t tree /dev/null)"
gp="$(printf 'deploy: site from %s (registry deadbeefcafe), 800 routes\n' "$PUB" | gitq -C "$R" commit-tree "$empty")"
git -C "$R" update-ref refs/heads/gh-pages "$gp"
git -C "$R" remote add origin "$R"; git -C "$R" fetch -q origin 'refs/heads/*:refs/remotes/origin/*'
cat > "$S/bin/gh" <<STUB
#!/usr/bin/env bash
case "\$*" in
  *pages/builds*) printf 'built\t%s\tno message\n' "$gp"; exit 0 ;;
  *releases/latest*) printf 'v0.21.0\t2026-10-12T10:23:43Z\n'; exit 0 ;;
esac
exit 1
STUB
chmod +x "$S/bin/gh"
live() {   # <commit the snapshot was read at | none>: the published site, whole but for the UI
  rm -rf "$W"; mkdir -p "$W/releases"
  jq -n --arg c "$PUB" '{schema: 1, commit: $c, dirty: false, release: {version: "0.21.0"}}' > "$W/build.json"
  printf '<html><body><dl data-trust><div data-trust-key="release" data-release="0.21.0"><dd>0.21.0</dd></div></dl>\n<a href="https://example.invalid/commit/%s">%s</a></body></html>\n' "$PUB" "$PUB" > "$W/index.html"
  jq -n '{schema: "release/v1", version: "0.21.0", tag: "v0.21.0"}' > "$W/releases/latest.json"
  [ "$1" = none ] || publication_snapshots "$ROOT" "$W" "$1" || { echo "    the stand-in snapshot could not be placed"; exit 1; }
}
gate() { ( cd "$R" && PATH="$S/bin:$PATH" env -u MAJORDOMUS_ROOT -u MAJORDOMUS_SHARE ./scripts/ci/pages-check --wait 0 "$@" ); }

live "$PUB"
expect_exit 0 gate
expect_grep "^ok    the live UI is the one read at ${PUB:0:12}: 2 of 2 page\(s\)"
expect_grep 'is being served with the UI read at the same commit'
live "$A"
expect_exit 10 gate
expect_grep "^FAIL  the live UI is not the one read at ${PUB:0:12}, is missing, or not all of it is served"
expect_grep "FAIL the published UI was read at ${A:0:10}, not at $PUB"
live none
expect_exit 10 gate
expect_grep "surface\.json is absent or unreadable \(status 404\): the publication holds no UI snapshot at $mount"
live "$PUB"
expect_exit 0 gate --offline
expect_grep '^ok    the live UI was not asked \(--offline\)'

# A tree that adds a snapshot declares what the *next* publication owes. The published commit's
# own model is what the publication is held to: asked for a snapshot that commit never declared,
# the gate refused every checkout from the commit that added the Cockpit's until its first
# deployment. So: a commit whose model has no Cockpit row is the published one, the trunk's
# newest commit adds the row, and the live site holds no Cockpit snapshot at all.
if grep -qE '^ *- surface: cockpit$' "$R/.ai/repo/ci/pages.yaml"; then
  cp "$R/.ai/repo/ci/pages.yaml" "$S/model-with.yaml"
  awk '/^ *- surface: cockpit$/ { skip = 1 } !skip { print } skip && /marker:/ { skip = 0 }' \
    "$S/model-with.yaml" > "$R/.ai/repo/ci/pages.yaml"
  gitq -C "$R" commit -qam 'published before the Cockpit snapshot was declared'
  PUB="$(git -C "$R" rev-parse HEAD)"
  cp "$S/model-with.yaml" "$R/.ai/repo/ci/pages.yaml"
  gitq -C "$R" commit -qam 'the Cockpit snapshot is declared'
  was="$gp"
  gp="$(printf 'deploy: site from %s (registry deadbeefcafe), 800 routes\n' "$PUB" | gitq -C "$R" commit-tree "$empty")"
  git -C "$R" update-ref refs/heads/gh-pages "$gp"
  git -C "$R" fetch -q origin '+refs/heads/*:refs/remotes/origin/*'
  sed -i.bak "s/$was/$gp/" "$S/bin/gh" && rm -f "$S/bin/gh.bak"
  live "$PUB"
  rm -rf "$W/cockpit"
  expect_exit 0 gate
  expect_grep "^ok    the published commit ${PUB:0:12} declares no cockpit snapshot; the one this tree declares is owed by the next publication$"
  expect_no_grep 'the live cockpit snapshot'
  echo "    pages-check: a snapshot the published commit never declared is owed by the next publication, not refused"
fi
stop_http
echo "    pages-check: ok for the UI read at the published commit, exit 10 for an older one and for none"

# ---------------------------------------------------------------- 4. the deploy
# scripts/site-deploy against a bare remote, the site a directory written here (as case 1041
# builds it), in a tree that carries the publication model and the topology.
D="$S/work"; mkdir -p "$D/scripts" "$D/.ai/repo/ci" "$D/docs/generated" "$D/site"
git init -q --bare "$S/remote.git"
cp "$ROOT/scripts/site-deploy" "$ROOT/scripts/pages" "$D/scripts/"
cp -R "$ROOT/lib" "$D/lib"; cp -R "$ROOT/bin" "$D/bin"; cp -R "$ROOT/share" "$D/share"
cp "$ROOT/.ai/manifest.yaml" "$ROOT/.ai/README.md" "$D/.ai/"
cp "$ROOT/.ai/repo/ci/pages.yaml" "$ROOT/.ai/repo/ci/gates.yaml" "$D/.ai/repo/ci/"
cp "$ROOT/docs/generated/web.json" "$D/docs/generated/web.json"
printf 'base_url = "https://example.invalid"\n' > "$D/site/config.toml"
printf 'site/public/\n' > "$D/.gitignore"
git init -q -b master "$D"; git -C "$D" remote add origin "$S/remote.git"
gitq -C "$D" add -A >/dev/null 2>&1; gitq -C "$D" commit -qm site; git -C "$D" push -q origin HEAD:master
HEADSHA="$(git -C "$D" rev-parse HEAD)"
built() {   # the site of HEAD as site/public, without what the site build does not compose
  rm -rf "$D/site/public"; mkdir -p "$D/site/public"
  printf '<a href="/commit/%s">source</a>\n' "$HEADSHA" > "$D/site/public/index.html"
  printf '{"commit":"%s"}\n' "$HEADSHA" > "$D/site/public/build.json"
}
deploy() { ( cd "$D" && env -u MAJORDOMUS_ROOT -u MAJORDOMUS_SHARE GIT_AUTHOR_NAME=t GIT_AUTHOR_EMAIL=t@example.com scripts/site-deploy --skip-build "$@" ); }

built
expect_exit 10 deploy
expect_grep "REFUSE site/public$mount holds no snapshot of the ui surface \(no surface\.json\), and the publication model says a publication holds one"
expect_grep "$producer && mkdir -p site/public$mount && cp -R $artifact/\. site/public$mount/"
[ -z "$(git -C "$D" ls-remote --heads origin gh-pages)" ] || { echo "    a refused deploy pushed gh-pages"; exit 1; }
publication_snapshots "$D" "$D/site/public" "$A" || { echo "    the stand-in snapshot could not be placed"; exit 1; }
expect_exit 10 deploy
expect_grep "REFUSE the snapshot of the ui surface at site/public$mount was read at ${A:0:10}, not at HEAD ${HEADSHA:0:7}"
publication_snapshots "$D" "$D/site/public" "$HEADSHA"
jq '.mount = "/elsewhere"' "$D/site/public$mount/surface.json" > "$S/j" && cp "$S/j" "$D/site/public$mount/surface.json"
expect_exit 10 deploy
expect_grep "REFUSE site/public$mount/surface\.json declares mount /elsewhere, not $mount"
[ -z "$(git -C "$D" ls-remote --heads origin gh-pages)" ] || { echo "    a refused deploy pushed gh-pages"; exit 1; }
publication_snapshots "$D" "$D/site/public" "$HEADSHA"
expect_exit 0 deploy
expect_grep 'pushed .* to origin/gh-pages: deploy: site from'
git -C "$D" fetch -q origin gh-pages
[ "$(git -C "$D" show "origin/gh-pages:${mount#/}/surface.json" | jq -r .built_from)" = "${HEADSHA:0:10}" ] \
  || { echo "    gh-pages does not carry the snapshot read at HEAD at $mount"; exit 1; }
# a model that cannot be read is not a model that declares nothing
printf 'version: 1\ndeploy:\n  snapshots:\n    - surface: ui\n' > "$D/.ai/repo/ci/pages.yaml"
expect_exit 10 deploy --allow-dirty
expect_grep 'REFUSE what a publication holds beside the site.s pages could not be read'
echo "    site-deploy: no snapshot, another commit's and a misplaced one refused; HEAD's published"

# ---------------------------------------------------------------- 5. wiring
awk '/^      - name: the UI it publishes$/{f=1} f && /^      - name: the release it offers$/{exit} f' "$WF" > "$S/step"
grep -q '^        run: scripts/pages verify-ui --commit "\$GITHUB_SHA"' "$S/step" \
  || { echo "    the pages workflow does not run verify-ui after a deploy"; exit 1; }
grep -q "if: steps.public.outcome == 'success'" "$S/step" \
  || { echo "    the workflow's UI probe does not wait for the site to serve the commit"; exit 1; }
if grep -q 'continue-on-error' "$S/step"; then echo "    the workflow's UI probe is advisory; a missing or stale UI must fail the deployment"; exit 1; fi
grep -q 'scripts/pages verify-ui --url "\$URL" --commit "\$source_sha" --timeout 0' "$ROOT/scripts/ci/pages-check" \
  || { echo "    pages-check (the pages-live gate) does not ask verify-ui of the published commit"; exit 1; }
grep -q 'scripts/pages snapshots' "$ROOT/scripts/site-deploy" \
  || { echo "    site-deploy does not read the model's snapshots through scripts/pages"; exit 1; }
if grep -vE '^[[:space:]]*#' "$ROOT/scripts/site-deploy" "$ROOT/scripts/ci/pages-check" | grep -qE '(public|URL[}"]*)/ui([/" ]|$)'; then
  echo "    a script writes the UI's mount itself instead of reading it from the model:"
  grep -nE '(public|URL[}"]*)/ui([/" ]|$)' "$ROOT/scripts/site-deploy" "$ROOT/scripts/ci/pages-check"; exit 1
fi
awk '/^  - id: pages-live$/{f=1} f && /^    summary:/{print; exit}' "$ROOT/.ai/repo/ci/gates.yaml" | grep -q 'the UI it holds' \
  || { echo "    the pages-live gate's summary does not state the UI"; exit 1; }
RULE="$ROOT/.ai/repo/rules/project/the-published-ui-is-the-trunks.v1.md"
grep -q '^class: blocking$' "$RULE" || { echo "    the rule is missing or not blocking"; exit 1; }
grep -q 'test/cases/2983_the_published_ui_is_the_trunks.sh' "$RULE" && grep -q 'scripts/ci/pages-check' "$RULE" \
  || { echo "    the rule's tests do not name this case and the gate"; exit 1; }
echo "    the workflow, the gate, the deploy, the gate's summary and the rule carry it"

echo "    the published UI is the trunk's"
