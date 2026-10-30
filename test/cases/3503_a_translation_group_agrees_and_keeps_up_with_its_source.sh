# majordomus-covers: script:scripts/site-check
# A translation group names the same members from every page, and a translation keeps up with
# the page it was translated from.
#
# The two-language form, [extra] alternate and alternate_lang, names one counterpart. The owner
# asked on 2026-10-19 for front matter that names every language version, and for the versions
# to be kept in sync automatically. A page now names its counterparts in [extra.translations]
# (lang = "route/", any number of languages), and site/templates/base.html emits one
# rel=alternate link per entry, plus its own (I3437). A translation names its source and the
# digest of the source's body it was made from, [extra] translated_from and source_digest, and
# the check recomputes it, so a source edited after its translation is loud (I3438). The
# digest is never rewritten for the translator: the re-sync is theirs.
#
# What is proved, against fixtures: check 21 of scripts/site-check (`--articles`) passes a
# three-language group written in the table, inline and dotted-key forms, and refuses a member
# that does not name the whole group back, a language listed twice, an article listing its own
# language, a stale digest (printing the digest to record, computed here independently, and
# leaving the file as it was), a missing digest, and a digest on an original. A fresh digest
# passes, and an edit to the source's front matter alone leaves it fresh. Last, the alternate
# links of base.html, rendered by zola, carry one hreflang per member of the group.
. "$ROOT/test/lib.sh"
# shellcheck source=../../lib/sha256.sh
. "$ROOT/lib/sha256.sh"
mj_sha256_tool 2>/dev/null || skip "no sha256sum or openssl"
CHECK="$ROOT/scripts/site-check"

# page FILE LANG [FRONT-MATTER-LINE...]: a page with two lines of body
page() {
  local f="$1" l="$2"; shift 2
  {
    printf '+++\ntitle = "%s"\ntemplate = "page.html"\n[extra]\nlang = "%s"\n' "${f##*/}" "$l"
    for x in "$@"; do printf '%s\n' "$x"; done
    printf '+++\nThe body of %s.\n\nIts second paragraph.\n' "${f##*/}"
  } > "$f"
}
# the digest of a page's body, the bytes after its closing fence, computed without the check
body_digest() { sed '1,/^+++$/d' "$1" | mj_sha256_hex; }
# fixture DIR: x (en), y (cs) and z (de), each naming the other two, three ways of writing it
fixture() {
  rm -rf "$1"; mkdir -p "$1/content"
  printf 'base_url = "https://example.invalid"\ndefault_language = "en"\n' > "$1/config.toml"
  printf '[[groups]]\nitems = [\n  { label = "x", href = "/x/" },\n]\n' > "$1/nav.toml"
  page "$1/content/x.md" en '[extra.translations]' 'cs = "y/"' 'de = "z/"'
  page "$1/content/y.md" cs 'translations = { en = "x/", de = "z/" }'
  page "$1/content/z.md" de 'translations.en = "x/"' 'translations.cs = "y/"'
}
run() { expect_exit "$1" "$CHECK" --articles "$PWD/$2/content" "$PWD/$2/nav.toml" "$PWD/$2/config.toml"; }

# ---------------------------------------------------------------- a three-language group passes
fixture fx
run 0 fx
expect_grep '^OK   articles .*\(1 of 3\) reads in every language the site declares \(cs de en\)'
expect_no_grep 'FAIL'

# ---------------------------------------------------------------- a member that does not name the group
fixture fx
page fx/content/z.md de '[extra.translations]' 'en = "x/"'
run 10 fx
expect_grep 'FAIL articles .*fx/content/y\.md names .*fx/content/z\.md as its de counterpart, and .*fx/content/z\.md does not name it back'
expect_grep 'FAIL articles .*fx/content/x\.md and .*fx/content/z\.md name each other but not the same group'
# the listed article is still owed a page in every declared language
page fx/content/x.md en '[extra.translations]' 'cs = "y/"'
run 10 fx
expect_grep 'FAIL articles .*fx/content/x\.md is an article the navigation lists, and it has no counterpart in de'

# ---------------------------------------------------------------- one page per language
fixture fx
page fx/content/x.md en '[extra.translations]' 'cs = "y/"' 'de = "z/"' 'cs = "z/"'
run 10 fx
expect_grep 'FAIL articles .*fx/content/x\.md lists cs twice in \[extra\.translations\]'
fixture fx
page fx/content/x.md en '[extra.translations]' 'cs = "y/"' 'de = "z/"' 'en = "x/"'
run 10 fx
expect_grep 'FAIL articles .*fx/content/x\.md lists its own language, en, as a counterpart'
# the two-language form beside a table must agree with it
fixture fx
page fx/content/x.md en 'alternate = "z/"' 'alternate_lang = "cs"' '[extra.translations]' 'cs = "y/"' 'de = "z/"'
run 10 fx
expect_grep 'FAIL articles .*fx/content/x\.md names z/ as its alternate, and its \[extra\.translations\] does not list it under cs'

# ---------------------------------------------------------------- a fresh digest passes
fixture fx
d="$(body_digest fx/content/x.md)"
[ "${#d}" = 64 ] || { echo "    no digest of the fixture source: '$d'"; exit 1; }
page fx/content/y.md cs 'translated_from = "x/"' "source_digest = \"$d\"" 'translations = { en = "x/", de = "z/" }'
run 0 fx
expect_grep '^OK   articles .*1 translation\(s\) match their source.s digest'
# an edit to the source's front matter alone is not an edit to what was translated
sed 's/^title = "x.md"$/title = "x, retitled"/' fx/content/x.md > x.tmp && mv x.tmp fx/content/x.md
grep -q 'x, retitled' fx/content/x.md || { echo "    the fixture source was not retitled"; exit 1; }
run 0 fx

# ---------------------------------------------------------------- a stale digest is refused, never rewritten
printf 'A paragraph written after the translation.\n' >> fx/content/x.md
new="$(body_digest fx/content/x.md)"
[ "$new" != "$d" ] || { echo "    the edited source kept its digest"; exit 1; }
before="$(cat fx/content/y.md)"
run 10 fx
expect_grep "FAIL articles .*fx/content/y\\.md is translated from .*fx/content/x\\.md, which has changed since the digest it records \\($d\\); re-sync the translation, then record source_digest = \"$new\""
[ "$(cat fx/content/y.md)" = "$before" ] || { echo "    the check rewrote the translation it judged"; exit 1; }

# ---------------------------------------------------------------- a missing digest, and a digest on an original
page fx/content/y.md cs 'translated_from = "x/"' 'translations = { en = "x/", de = "z/" }'
run 10 fx
expect_grep "FAIL articles .*fx/content/y\\.md is translated from .*fx/content/x\\.md and records no source_digest; .*record source_digest = \"$new\""
fixture fx
page fx/content/x.md en "source_digest = \"$d\"" '[extra.translations]' 'cs = "y/"' 'de = "z/"'
run 10 fx
expect_grep 'FAIL articles .*fx/content/x\.md records a source_digest and names no translated_from'
# a source outside the group is not the page it was translated from
fixture fx
page fx/content/w.md en
page fx/content/y.md cs 'translated_from = "w/"' "source_digest = \"$d\"" 'translations = { en = "x/", de = "z/" }'
run 10 fx
expect_grep 'FAIL articles .*fx/content/y\.md is translated from .*fx/content/w\.md, which is not one of its counterparts'

# ---------------------------------------------------------------- base.html links every member
if command -v zola >/dev/null; then
  Z="$PWD/zsite"; mkdir -p "$Z/templates" "$Z/content"
  printf 'base_url = "https://example.invalid"\ntitle = "fixture"\n' > "$Z/config.toml"
  # the alternate links of the real base.html, from its translations branch to its endif
  block="$(awk '/page\.extra\.translations is defined/ { on = 1 } on { print } on && /^[{]%- endif %[}]/ { exit }' \
    "$ROOT/site/templates/base.html")"
  printf '%s\n' "$block" | grep -q 'page.extra.alternate is defined' \
    || { echo "    base.html no longer reads the two-language form beside the table:"; printf '%s\n' "$block"; exit 1; }
  { printf '{%%- set page_lang = page.extra.lang -%%}{%%- set page_url = page.permalink -%%}\n'; printf '%s\n' "$block"; } \
    > "$Z/templates/page.html"
  printf '{{ section.title }}\n' > "$Z/templates/index.html"
  printf '+++\ntitle = "home"\n+++\n' > "$Z/content/_index.md"
  cp fx/content/x.md fx/content/y.md fx/content/z.md "$Z/content/"
  printf '+++\ntitle = "p"\ntemplate = "page.html"\n[extra]\nlang = "en"\nalternate = "q/"\nalternate_lang = "cs"\n+++\nbody\n' > "$Z/content/p.md"
  ( cd "$Z" && run_quiet "$PWD/../zola.err" zola build --force -o "$Z/public" >/dev/null ) \
    || { echo "    zola could not render the alternate links:"; cat zola.err; exit 1; }
  for want in 'hreflang="en" href="https://example.invalid/x/"' 'hreflang="cs" href="https://example.invalid/y/"' \
    'hreflang="de" href="https://example.invalid/z/"'; do
    grep -qF "$want" "$Z/public/x/index.html" || { echo "    /x/ lacks <link rel=alternate $want>:"; cat "$Z/public/x/index.html"; exit 1; }
  done
  [ "$(grep -c 'rel="alternate"' "$Z/public/x/index.html")" = 3 ] \
    || { echo "    /x/ carries other than one alternate link per member:"; cat "$Z/public/x/index.html"; exit 1; }
  grep -qF 'hreflang="cs" href="https://example.invalid/q/"' "$Z/public/p/index.html" \
    || { echo "    the two-language form no longer renders its alternate:"; cat "$Z/public/p/index.html"; exit 1; }
else
  echo "    (zola absent: the rendered alternate links are not checked)"
fi
