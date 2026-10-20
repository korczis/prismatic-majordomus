# majordomus-covers: script:scripts/site-check
# Every article reads in every language the site declares, and its counterparts are reciprocal.
#
# site/templates/base.html emits a rel=alternate link from a page's [extra] alternate and
# alternate_lang, and scripts/ci/link-check refuses a counterpart that is not built. Neither
# saw the other direction: an English article could name its Czech counterpart while the Czech
# page named nothing, or named a page in its own language, and an article could be listed in
# the navigation with no counterpart at all. On 2026-10-19 the owner asked that the articles
# be interlinked in every language, and that this be enforced (I3436).
#
# What is proved, against fixtures rather than this checkout's articles: check 21 of
# scripts/site-check (`--articles <dir> <nav.toml> <config.toml>`) passes a reciprocal pair
# and a page that declares no language, and refuses, naming the file, a one-way alternate, a
# dangling target, a counterpart listed under another language than its own, a counterpart in
# the article's own language, and an article the navigation lists with no counterpart. A site
# that declares one language owes no counterpart. Last, the real tree passes.
. "$ROOT/test/lib.sh"
CHECK="$ROOT/scripts/site-check"

# page FILE LANG [FRONT-MATTER-LINE...]: a page with one line of body; LANG "" declares none
page() {
  local f="$1" l="$2"; shift 2
  {
    printf '+++\ntitle = "%s"\ntemplate = "page.html"\n[extra]\n' "${f##*/}"
    if [ -n "$l" ]; then printf 'lang = "%s"\n' "$l"; fi
    for x in "$@"; do printf '%s\n' "$x"; done
    printf '+++\nThe body of %s.\n' "${f##*/}"
  } > "$f"
}
# fixture DIR: a reciprocal pair, a page that declares no language, a navigation listing all
# three and a config whose default language is English
fixture() {
  rm -rf "$1"; mkdir -p "$1/content"
  printf 'base_url = "https://example.invalid"\ndefault_language = "en"\n' > "$1/config.toml"
  cat > "$1/nav.toml" <<'EOF'
# an href in a comment is not a navigation entry: href = "/ghost/"
[[groups]]
label = "Articles"
items = [
  { label = "An article", href = "/a/" },
  { label = "The same article, in Czech", href = "/b/" },
  { label = "A page", href = "/plain/" },
]
EOF
  page "$1/content/a.md" en 'alternate = "b/"' 'alternate_lang = "cs"'
  page "$1/content/b.md" cs 'alternate = "a/"' 'alternate_lang = "en"'
  page "$1/content/plain.md" ""
}
run() { expect_exit "$1" "$CHECK" --articles "$PWD/$2/content" "$PWD/$2/nav.toml" "$PWD/$2/config.toml"; }

# ---------------------------------------------------------------- a reciprocal pair passes
fixture fx
run 0 fx
expect_grep '^OK   articles .*\(2 of 2\) reads in every language the site declares \(cs en\)'
expect_no_grep 'FAIL'
expect_no_grep 'plain\.md'

# ---------------------------------------------------------------- a one-way alternate
fixture fx
page fx/content/c.md cs 'alternate = "a/"' 'alternate_lang = "en"'
run 10 fx
expect_grep 'FAIL articles .*fx/content/c\.md names .*fx/content/a\.md as its en counterpart, and .*fx/content/a\.md does not name it back'
expect_no_grep 'FAIL articles .*fx/content/b\.md'

# ---------------------------------------------------------------- a dangling target
fixture fx
page fx/content/a.md en 'alternate = "nowhere/"' 'alternate_lang = "cs"'
run 10 fx
expect_grep 'FAIL articles .*fx/content/a\.md names nowhere/ as its cs counterpart, and no page under .*fx/content is at that route'
# and the page it left behind is now one-way
expect_grep 'FAIL articles .*fx/content/b\.md names .*fx/content/a\.md as its en counterpart, and .*a\.md does not name it back'

# ---------------------------------------------------------------- a language mismatch
fixture fx
page fx/content/a.md en 'alternate = "b/"' 'alternate_lang = "de"'
run 10 fx
expect_grep 'FAIL articles .*fx/content/a\.md names .*fx/content/b\.md as its de counterpart, and .*fx/content/b\.md is written in cs'
expect_grep 'FAIL articles .*fx/content/a\.md is an article the navigation lists, and it has no counterpart in cs'

# ---------------------------------------------------------------- a counterpart in the same language
fixture fx
page fx/content/b.md en 'alternate = "a/"' 'alternate_lang = "en"'
run 10 fx
expect_grep 'FAIL articles .*fx/content/a\.md names .*fx/content/b\.md as its cs counterpart, and both are written in en'
expect_grep 'FAIL articles .*fx/content/b\.md lists its own language, en, as a counterpart'

# ---------------------------------------------------------------- a missing counterpart
fixture fx
page fx/content/a.md en
page fx/content/b.md cs
run 10 fx
expect_grep 'FAIL articles .*fx/content/a\.md is an article the navigation lists, and it has no counterpart in cs'
expect_grep 'FAIL articles .*fx/content/b\.md is an article the navigation lists, and it has no counterpart in en'
expect_no_grep 'plain\.md'
# an article the navigation does not list owes nothing, and an href in a comment lists nothing
page fx/content/b.md cs 'alternate = "ghost/"' 'alternate_lang = "en"'
page fx/content/ghost.md en 'alternate = "b/"' 'alternate_lang = "cs"'
page fx/content/a.md ""
run 0 fx
expect_grep '^OK   articles .*\(1 of 2\)'

# ---------------------------------------------------------------- one language owes no counterpart
fixture fx
rm fx/content/b.md
page fx/content/a.md en
run 0 fx
expect_grep '^OK   articles .*declares one language \(en\)'

# ---------------------------------------------------------------- the real tree
expect_exit 0 "$CHECK" --articles "$ROOT/site/content-src" "$ROOT/site/data/nav.toml" "$ROOT/site/config.toml"
expect_grep '^OK   articles '
expect_exit 12 "$CHECK" --articles "$PWD/no-such-dir" "$ROOT/site/data/nav.toml" "$ROOT/site/config.toml"
