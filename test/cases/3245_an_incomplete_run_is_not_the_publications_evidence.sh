# majordomus-covers: none
# A run cancelled midway is not the publication's evidence while a complete one is near (I2845).
# On 2026-10-20 the master validate of 82ac259d0ae was cancelled after its suite and coverage
# jobs started. Its evidence artifact still uploaded, naming both reports as absent, and it was
# the nearest ancestor of the next master. Two Pages deploys (39438897759, 39451372815) read its
# ledger and refused "215 claim(s) would be published as not run", every one a claim the suite
# would have proven. `scripts/pages evidence` now reads the nearest run that reported from every
# runner, and an incomplete run stands in only when no complete one is among the nearest.
# Through the real script, against manifests in a temporary directory, read-only:
#
#   0  the CI path: of two evidence artifacts at the nearest commit, the incomplete one listed
#      first, the complete one is downloaded and read (a stand-in `gh` serves both)
#   1  a nearer incomplete run and an older complete one: the complete one is read, and the
#      incomplete one is named for what it lacks
#   2  only an incomplete run: it stands in, and says so
#   3  one complete run: read as before, with no word about passing anything over
. "$ROOT/test/lib.sh"

W="$T/w"
mkdir -p "$W"
head="$(git -C "$ROOT" rev-parse HEAD)"

manifest() {     # manifest <dir> <produced_at> <absent json array>
  mkdir -p "$1"
  jq -n --arg c "$head" --arg at "$2" --argjson absent "$3" \
    '{schema: 1, commit: $c, produced_at: $at, absent: $absent, panels: [],
      working_tree: "clean", report_tree: "clean", totals: {outcomes: {}}}' > "$1/manifest.json"
}
manifest "$W/incomplete" "2026-10-20T08:29:00Z" '["suite", "coverage"]'
manifest "$W/complete" "2026-10-20T07:00:00Z" '[]'

# ---------------------------------------------------------------- 0. the CI path
# A stand-in `gh` answers the two calls the path makes: the artifact list, already filtered to
# "id<TAB>commit" as gh's --jq would, and each artifact's zip. Both artifacts name HEAD, so the
# nearest-first order falls to the id, and the incomplete one (1) is offered first.
for k in incomplete complete; do
  python3 -I -c 'import sys,zipfile; z=zipfile.ZipFile(sys.argv[1],"w"); z.write(sys.argv[2],"manifest.json"); z.close()' \
    "$W/$k.zip" "$W/$k/manifest.json" || { echo "    could not zip the $k manifest"; exit 1; }
done
mkdir -p "$W/bin"
cat > "$W/bin/gh" <<EOF
#!/usr/bin/env bash
case "\$*" in
  *"artifacts?name=evidence"*) printf '1\t%s\n2\t%s\n' "$head" "$head" ;;
  *"artifacts/1/zip"*) cat "$W/incomplete.zip" ;;
  *"artifacts/2/zip"*) cat "$W/complete.zip" ;;
  *) exit 1 ;;
esac
EOF
chmod +x "$W/bin/gh"
PATH="$W/bin:$PATH" GITHUB_REPOSITORY=example/repo expect_exit 0 \
  "$ROOT/scripts/pages" evidence --commit "$head" --out "$W/zero.json"
expect_grep "passed over nearer incomplete evidence"
[ "$(jq -r .produced_at "$W/zero.json")" = "2026-10-20T07:00:00Z" ] \
  || { echo "    the CI path read the incomplete artifact although a complete one was listed"; jq . "$W/zero.json"; exit 1; }

# ---------------------------------------------------------------- 1. the complete run is read
expect_exit 0 "$ROOT/scripts/pages" evidence --commit "$head" \
  --from "$W/incomplete" --from "$W/complete" --out "$W/one.json"
expect_grep "passed over nearer incomplete evidence \(the run of ${head:0:12} has no report from suite, coverage\)"
[ "$(jq -r .produced_at "$W/one.json")" = "2026-10-20T07:00:00Z" ] \
  || { echo "    the incomplete run was read although a complete one was given"; jq . "$W/one.json"; exit 1; }

# ---------------------------------------------------------------- 2. only an incomplete run
expect_exit 0 "$ROOT/scripts/pages" evidence --commit "$head" --from "$W/incomplete" --out "$W/two.json"
expect_grep "no complete run among the 1 nearest; the nearest stands in: the run of ${head:0:12} has no report from suite, coverage"
[ "$(jq -r .produced_at "$W/two.json")" = "2026-10-20T08:29:00Z" ] \
  || { echo "    a lone incomplete run did not stand in"; jq . "$W/two.json"; exit 1; }
jq -e '.unconfirmed | any(test("no report from suite, coverage"))' "$W/two.json" >/dev/null \
  || { echo "    the stand-in does not say what it lacks"; jq .unconfirmed "$W/two.json"; exit 1; }

# ---------------------------------------------------------------- 3. one complete run
expect_exit 0 "$ROOT/scripts/pages" evidence --commit "$head" --from "$W/complete" --out "$W/three.json"
case "$LAST_OUT" in *"passed over"*|*"stands in"*) echo "    a lone complete run was reported as a choice"; echo "$LAST_OUT"; exit 1 ;; esac
[ "$(jq -r .produced_at "$W/three.json")" = "2026-10-20T07:00:00Z" ] \
  || { echo "    the complete run was not read"; exit 1; }

echo "    the nearest complete run is the publication's evidence; an incomplete one stands in only alone, and says what it lacks"
