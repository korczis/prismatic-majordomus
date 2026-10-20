# majordomus-covers: none
# majordomus-negative: rules
# Every language of the site is derived from the English build and a translation memory.
#
# ADR 0218: the copy of a page under /<lang>/ is the English page with each segment the memory
# knows replaced, its internal links pointed at that language, its language and canonical URL
# declared, and the language switch pointed at each copy. A translation is admitted only with
# the placeholders of its English, and a segment the memory does not know stays English and
# is listed as missing. Each of those is asserted here over a two-page fixture site, built
# by the shipped pass; nothing in the repository is written.
. "$ROOT/test/lib.sh"
PASS="$ROOT/scripts/lib/site-i18n.mjs"
[ -f "$PASS" ] || { echo "    scripts/lib/site-i18n.mjs is missing"; exit 1; }

SITE="$PWD/site"
mkdir -p "$SITE/public/guide" "$SITE/i18n/cs/memory"
printf '{"schema":"site-languages/v1","source":"en","languages":["en","cs"],"names":{"en":"English","cs":"Czech"}}\n' \
  > "$SITE/i18n/languages.json"
page() {
  cat <<HTML
<!doctype html>
<html lang="en"><head><title>Getting started</title>
<link rel="canonical" href="https://example.test$1"></head>
<body><nav translate="no"><a href="https://example.test$1" data-i18n-keep data-i18n-current data-i18n-lang="en">en</a><a href="https://example.test$1" data-i18n-keep data-i18n-current data-i18n-lang="cs">cs</a></nav>
<div data-i18n-notice hidden><p>Translated automatically.</p></div>
<main><h1>Getting started</h1>
<p>Read <a href="https://example.test/guide/">the guide</a> before you run <code>majordomus init</code>.</p>
<p>An untranslated sentence.</p>
<pre><code>Code is never translated.</code></pre>
<p><a href="https://example.test/releases/latest.json">Release metadata</a></p></main></body></html>
HTML
}
page / > "$SITE/public/index.html"
page /guide/ > "$SITE/public/guide/index.html"
# a Zola alias, which carries no words and only a target
mkdir -p "$SITE/public/old"
printf '<!doctype html>\n<meta charset="utf-8">\n<title>Redirect</title>\n<meta http-equiv="refresh" content="0; url=https://example.test/guide/">\n' \
  > "$SITE/public/old/index.html"
printf '<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n  <url><loc>https://example.test/</loc></url>\n  <url><loc>https://example.test/guide/</loc></url>\n</urlset>\n' \
  > "$SITE/public/sitemap.xml"

# The memory, keyed as the pass keys it: a good translation, and one that drops a placeholder.
node --input-type=module - "$PASS" "$SITE/i18n/cs/memory" <<'JS' || { echo "    the memory could not be written"; exit 1; }
const [pass, dir] = process.argv.slice(2);
const { keyOf, saveMemory } = await import(pass);
const entries = [
  ['Getting started', 'Začínáme'],
  ['Read <t1>the guide</t1> before you run <t2/>.', 'Než spustíte <t2/>, přečtěte si <t1>návod</t1>.'],
  ['Release metadata', 'Metadata vydání'],
  ['Translated automatically.', 'Přeloženo automaticky.'],
  // a translation that lost its code placeholder: refused, the English stays
  ['An untranslated sentence.', 'Nepřeložená věta <t9/>.'],
];
saveMemory(dir, new Map(entries.map(([en, cs]) => [keyOf(en), { k: keyOf(en), en, cs }])));
JS

out="$(node "$PASS" build "$SITE/public" "$SITE/i18n" https://example.test "$PWD/missing.json")" \
  || { echo "    the pass failed: $out"; exit 1; }
CS="$SITE/public/cs/index.html"
[ -s "$CS" ] && [ -s "$SITE/public/cs/guide/index.html" ] || { echo "    no Czech copy of every page"; exit 1; }
grep -q '<html lang="cs">' "$CS" || { echo "    the Czech copy does not declare its language"; exit 1; }
grep -q '<title>Začínáme</title>' "$CS" || { echo "    the title was not translated"; exit 1; }
grep -q 'Než spustíte <code>majordomus init</code>, přečtěte si <a href="https://example.test/cs/guide/">návod</a>.' "$CS" \
  || { echo "    the placeholders were not restored, or the link was not pointed at the Czech copy"; exit 1; }
grep -q '<p>An untranslated sentence.</p>' "$CS" \
  || { echo "    a translation that dropped a placeholder was admitted"; exit 1; }
grep -q 'Code is never translated.' "$CS" || { echo "    code was touched"; exit 1; }
grep -q 'href="https://example.test/releases/latest.json">Metadata vydání' "$CS" \
  || { echo "    a link to a file that is not a page was rewritten"; exit 1; }
grep -q '<link rel="canonical" href="https://example.test/cs/">' "$CS" \
  || { echo "    the Czech copy is not its own canonical URL"; exit 1; }
grep -q 'hreflang="cs" href="https://example.test/cs/"' "$SITE/public/index.html" \
  || { echo    "    the English page has no alternate naming its Czech copy"; exit 1; }
grep -q 'href="https://example.test/cs/" data-i18n-keep data-i18n-current data-i18n-lang="cs" aria-current="page"' "$CS" \
  || { echo "    the switch does not point at the Czech copy and mark it current"; exit 1; }
grep -q 'href="https://example.test/" data-i18n-keep data-i18n-current data-i18n-lang="en">' "$CS" \
  || { echo "    the switch does not lead back to the English original"; exit 1; }
grep -q '<div data-i18n-notice><p>Přeloženo automaticky.</p>' "$CS" \
  || { echo "    the translation notice is not revealed on the Czech copy"; exit 1; }
grep -q '<div data-i18n-notice hidden>' "$SITE/public/index.html" \
  || { echo "    the translation notice is shown on the English original"; exit 1; }
jq -e '.cs | map(.en) | index("An untranslated sentence.") != null' "$PWD/missing.json" >/dev/null \
  || { echo "    the refused segment is not listed as missing"; exit 1; }
jq -e '.languages.cs.translated > 0 and .languages.cs.coverage < 100' "$SITE/public/cs/i18n.json" >/dev/null \
  || { echo "    the coverage is not published"; exit 1; }

grep -q '<title>Redirect</title>' "$SITE/public/cs/old/index.html" \
  && grep -q 'url=https://example.test/cs/guide/' "$SITE/public/cs/old/index.html" \
  || { echo "    an alias is not carried as a redirect to its target's copy"; exit 1; }
grep -q '<loc>https://example.test/cs/guide/</loc>' "$SITE/public/sitemap.xml" \
  || { echo "    the sitemap does not list the Czech copy"; exit 1; }
grep -q 'hreflang="cs" href="https://example.test/cs/guide/"' "$SITE/public/sitemap.xml" \
  || { echo "    the sitemap does not name the copies as alternates"; exit 1; }

# A second build over the same input writes the same bytes.
cp -R "$SITE/public" "$PWD/before"
node "$PASS" build "$SITE/public" "$SITE/i18n" https://example.test "$PWD/missing.json" >/dev/null
diff -r "$PWD/before" "$SITE/public" >/dev/null || { echo "    a second pass changed the site"; exit 1; }
