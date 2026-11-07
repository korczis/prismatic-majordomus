# majordomus-covers: none
# claims: derived-data-current
# The live site is probed in a real browser after every deploy and release (I4530):
# scripts/release-probe renders the homepage in Chromium with its cache bypassed and holds what
# it says to what was published — the trust strip's data-release, /releases/latest.json and
# GitHub's newest release name one version and the navbar badge names it; the hero names the
# newest published pre-release ahead of that version and no other; the footer names
# /build.json's commit; with --expect, the tag just published is GitHub's newest and the page
# names it. Until 2026-11-06 the only live reading was curl and sed over the stable channel
# (scripts/pages release-live): the hero named a superseded pre-release for hours after every
# release with every check green. Rule: project.the-site-offers-the-release-it-published.
#
# What is proved, over a fixture the probe serves at the site's own origin (no network; GitHub
# answered from a file in the API's own shape), with a screenshot and result.json written:
#   1  a page that agrees with its JSON and with GitHub passes, and the files are written
#   2  a hero naming a superseded pre-release is refused, naming both versions
#   3  a footer commit that is not /build.json's is refused, naming both
#   4  a trust strip that is not /releases/latest.json is refused
#   5  --expect of a tag GitHub has not published is refused
#   6  a GitHub listing that cannot be read is NOT DETERMINED (12), never a pass
# It needs a browser: without one it skips outside CI (the convention of case 337).
. "$ROOT/test/lib.sh"
PROBE="$ROOT/scripts/release-probe"
[ -x "$PROBE" ] || { echo "    scripts/release-probe is missing or not executable"; exit 1; }
mj_runs node || skip "$(mj_runs_why node)"
[ -d "$ROOT/node_modules/playwright" ] || { [ "${CI:-}" = true ] && { echo "    playwright absent under CI"; exit 1; }; skip "playwright absent (npm ci)"; }

F="$PWD/fixture"; OUT="$PWD/out"
SHA=0123456789abcdef0123456789abcdef01234567
OTHER=fedcba9876543210fedcba9876543210fedcba98
fresh() {
  rm -rf "$F" "$OUT"; mkdir -p "$F/releases"
  cat > "$F/index.html" <<HTML
<!doctype html><html><head><title>Fixture</title></head><body>
<nav><a href="/">Fixture</a> <span>v0.30.2</span></nav>
<main>
<section id="hero"><p><a href="/changelog/" data-prerelease="1.0.0-alpha.10">1.0.0-alpha.10</a> (pre-release) · stable <b>0.30.2</b></p><h1>Your agents say the work is done.</h1></section>
<section id="trust"><dl>
<div data-trust-key="release" data-release="0.30.2"><dt>release</dt><dd>0.30.2</dd><dd data-prerelease="1.0.0-alpha.10">pre-release 1.0.0-alpha.10</dd></div>
<div data-trust-key="commit"><dt>commit</dt><dd>0123456</dd></div>
</dl></section>
</main>
<footer><p>Built from <a href="https://example.test/commit/$SHA" data-blackboard-commit="$SHA">0123456</a></p></footer>
</body></html>
HTML
  printf '{"version":"0.30.2","tag":"v0.30.2","channel":"stable"}\n' > "$F/releases/latest.json"
  printf '{"schema":1,"commit":"%s","dirty":false,"release":{"version":"0.30.2"}}\n' "$SHA" > "$F/build.json"
  cat > "$PWD/github.json" <<'JSON'
[
  {"tag_name":"v1.0.0-alpha.10","prerelease":true,"draft":false,"published_at":"2026-11-06T17:09:12Z"},
  {"tag_name":"v1.0.0-alpha.9","prerelease":true,"draft":false,"published_at":"2026-10-28T12:36:48Z"},
  {"tag_name":"v0.30.2","prerelease":false,"draft":false,"published_at":"2026-10-20T10:00:00Z"},
  {"tag_name":"v9.9.9","prerelease":false,"draft":true,"published_at":null}
]
JSON
}
probe() { # <extra args...>; prints the exit code; output in out.txt
  rc=0; "$PROBE" --public "$F" --github "$PWD/github.json" --out "$OUT" "$@" > out.txt 2>&1 || rc=$?
  if grep -q '^SKIP release-probe: no browser' out.txt; then skip "no browser (install Chrome, set CHROME_BIN, or: npx playwright install chromium)"; fi
  echo "$rc"
}
expect_probe() { # <exit> <pattern> <what>
  rc="$(probe "${@:4}")"
  [ "$rc" = "$1" ] || { echo "    $3: exit $rc, expected $1"; cat out.txt; exit 1; }
  grep -qE -- "$2" out.txt || { echo "    $3: no line matching '$2'"; cat out.txt; exit 1; }
}

# ---------------------------------------------------------------- 1  agreement, and the record of it
fresh
expect_probe 0 'ok — the homepage, its JSON and GitHub name the same releases and commit' "a page that agrees with what was published"
[ -s "$OUT/home.png" ] || { echo "    no screenshot was written"; ls -la "$OUT"; exit 1; }
jq -e '.verdict == "ok" and .readings.release == "0.30.2" and .readings.hero_prerelease == "1.0.0-alpha.10" and .readings.nav_badge == "v0.30.2" and .readings.github.prerelease_ahead == "v1.0.0-alpha.10" and (.findings | length) == 0' "$OUT/result.json" > /dev/null \
  || { echo "    result.json does not record the readings and the verdict:"; cat "$OUT/result.json"; exit 1; }
# the draft is never counted as published
jq -e '.readings.github.newest == "v1.0.0-alpha.10"' "$OUT/result.json" > /dev/null || { echo "    a draft release was counted as published"; exit 1; }

# ---------------------------------------------------------------- 2  a superseded pre-release on the hero
fresh; sed -i.bak 's/1\.0\.0-alpha\.10/1.0.0-alpha.9/g' "$F/index.html"
expect_probe 10 'FAIL the hero names the pre-release 1\.0\.0-alpha\.9, and the newest published pre-release ahead of 0\.30\.2 is v1\.0\.0-alpha\.10' "a hero naming the previous pre-release"
expect_probe 10 'FAIL the hero names the superseded pre-release\(s\) 1\.0\.0-alpha\.9' "the superseded pre-release named on the hero"

# ---------------------------------------------------------------- 3  a footer commit that is not the build's
fresh; sed -i.bak "s/data-blackboard-commit=\"$SHA\"/data-blackboard-commit=\"$OTHER\"/" "$F/index.html"
expect_probe 10 "FAIL the footer says the page was built from ${OTHER:0:11} and /build.json names ${SHA:0:11}" "a footer naming another commit"

# ---------------------------------------------------------------- 4  a trust strip that is not latest.json
fresh; printf '{"version":"0.30.3","tag":"v0.30.3","channel":"stable"}\n' > "$F/releases/latest.json"
expect_probe 10 'FAIL the homepage offers 0\.30\.2 and /releases/latest\.json names 0\.30\.3' "a trust strip behind latest.json"

# ---------------------------------------------------------------- 5  --expect of a tag that is not GitHub's newest
fresh
expect_probe 10 "FAIL GitHub's newest release is v1\.0\.0-alpha\.10, not v1\.0\.0-alpha\.11" "--expect of an unpublished tag" --expect v1.0.0-alpha.11
expect_probe 0 'ok — ' "--expect of the tag just published" --expect v1.0.0-alpha.10

# ---------------------------------------------------------------- 6  GitHub unreadable is not a pass
fresh; printf 'not json\n' > "$PWD/github.json"
expect_probe 12 "NOT DETERMINED GitHub's releases of .* could not be read" "an unreadable GitHub listing"
