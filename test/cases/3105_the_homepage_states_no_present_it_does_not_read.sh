# majordomus-covers: none
# claims: none
# The landing page states no present tense it does not read (ADR 0166, decision 5).
#
# The homepage said "What is shipped, what is ready and what is in progress, straight from the
# repository" over counts of plan stamps, its argument said "Nothing replans" and "No command
# runs this today", a link promised the roadmap "wave by wave" and the hero drew progress bars
# the roadmap says it does not have. Each was a second account of the repository, typed beside
# the data the page already renders. scripts/ci/homepage-check `present` refuses the vocabulary
# of such a sentence in the copy files' string values and in the homepage template's text.
#
# What is proved, over a tree this case builds so it runs in seconds:
#   refused   a planted sentence carrying each form of the vocabulary, in each copy file and in
#             the template's markup, in a string a {% set %} types, and in an aria-label, each
#             named with its file, its line and the form
#   allowed   the same words in a TOML comment, in a TOML key and in a {# template comment #}
#   unread    a copy file or the template that cannot be read is a finding, never a clean pass
#   real      the checkout's own marketing.toml, manifesto.toml, homepage.toml and
#             site/templates/index.html pass, and the template reads its plan and release
#             figures from the generated data rather than from a sentence or a milestone bar
. "$ROOT/test/lib.sh"
CHECK="$ROOT/scripts/ci/homepage-check"
[ -x "$CHECK" ] || { echo "    scripts/ci/homepage-check is missing or not executable"; exit 1; }
mj_runs python3 || skip "$(mj_runs_why python3)"

F="$PWD/fixture"
# the least tree homepage-check reads before it judges anything; the copy and the template are
# written per step
fresh() {
  rm -rf "$F"
  mkdir -p "$F/site/data/registry" "$F/site/data/generated" "$F/site/public" "$F/site/templates" "$F/docs/generated"
  printf 'order = ["hero"]\n\n[promises]\n"unattended" = ""\n' > "$F/site/data/homepage.toml"
  printf '[indexing]\nunlisted = []\n' > "$F/site/data/nav.toml"
  printf '%s\n' '{"features":[],"domains":[],"providers":[],"evidence":{"available":false,"claims":{}}}' > "$F/site/data/registry/product.json"
  printf '{}\n' > "$F/site/data/registry/distribution.json"
  printf '{"states":{}}\n' > "$F/site/data/registry/design.json"
  printf '{}\n' > "$F/site/data/generated/capabilities.json"
  printf '{"surfaces":[]}\n' > "$F/docs/generated/web.json"
  printf '<html><body><main><section id="hero"><a href="/x/">x</a></section></main></body></html>\n' > "$F/site/public/index.html"
  printf '[hero]\ntitle = "Your agents say the work is done."\n' > "$F/site/data/marketing.toml"
  printf '[intro]\nlead = "The repository decides what is done."\n' > "$F/site/data/manifesto.toml"
  printf '<section id="hero"><h1>{{ m.hero.title }}</h1></section>\n' > "$F/site/templates/index.html"
}
judge() { # <exit> <pattern> <what>
  rc=0; MJ_ROOT="$F" "$CHECK" --only present > out.txt 2>&1 || rc=$?
  [ "$rc" = "$1" ] || { echo "    $3: exit $rc, expected $1"; cat out.txt; exit 1; }
  grep -qE "$2" out.txt || { echo "    $3: no line matching '$2'"; cat out.txt; exit 1; }
  [ "$(grep -c '^\(OK\|FAIL\) ' out.txt)" -ge 1 ] || { echo "    $3: the check printed no verdict"; cat out.txt; exit 1; }
}

fresh
judge 0 '^OK   present +no hand-written sentence states how the work stands' "a clean tree"

# --- refused: each form, planted as a sentence of the copy
while IFS='|' read -r form sentence; do
  [ -n "$form" ] || continue
  fresh; printf 'lead = "%s"\n' "$sentence" >> "$F/site/data/marketing.toml"
  judge 10 "^FAIL present +site/data/marketing.toml:3 states the present \\('$form'\\)" "the form '$form'"
done <<'EOF'
today|Parts of it run today.
not yet|A commit is not yet traced to its intent.
nothing ... yet|Nothing replans the work yet.
no command ... today|No command runs the opposition pass today.
shipped|What is shipped is on the matrix.
currently|The plan currently has no deriver.
not built|Built: the record. Not built: what acts on it.
under way|Milestones under way.
in progress|What is in progress, straight from the repository.
so far|So far the comparator runs alone.
scheduled|What is scheduled, wave by wave.
right now|Right now nothing derives a plan.
EOF

# each copy file is read, and so is the template's text
fresh; printf '[loop]\nkicker = "a control loop — mostly not built"\n' >> "$F/site/data/manifesto.toml"
judge 10 "site/data/manifesto.toml:4 states the present \\('not built'\\)" "a planted sentence in manifesto.toml"
fresh; printf '\n[note]\ntext = "Every word here is empty today."\n' >> "$F/site/data/homepage.toml"
judge 10 "site/data/homepage.toml:7 states the present \\('today'\\)" "a planted sentence in homepage.toml"
fresh; printf '<a href="/roadmap/">What is scheduled, wave by wave</a>\n' >> "$F/site/templates/index.html"
judge 10 "site/templates/index.html:2 states the present \\('scheduled'\\)" "a planted sentence in the template's markup"
fresh; printf '{%%- set figures = [{"l": "issues currently open"}] -%%}\n' >> "$F/site/templates/index.html"
judge 10 "site/templates/index.html:2 states the present \\('currently'\\)" "a label typed into a set statement"
fresh; printf '<ul aria-label="Milestones under way"></ul>\n' >> "$F/site/templates/index.html"
judge 10 "site/templates/index.html:2 states the present \\('under way'\\)" "an aria-label stating the present"

# --- allowed: a comment is not copy, a key is not a sentence, a template comment is not text
fresh; printf '# what is shipped today is on the matrix\nshipped_title = "The commands, each declared once"\n' >> "$F/site/data/marketing.toml"
judge 0 '^OK   present ' "a TOML comment and a key"
fresh; printf '{#- the strip: what this repository measures today, not yet built -#}\n' >> "$F/site/templates/index.html"
judge 0 '^OK   present ' "a template comment"
# the same word inside a longer one is not the word
fresh; printf 'lead = "Yesterday is history; shippedness is not a word."\n' >> "$F/site/data/marketing.toml"
judge 0 '^OK   present ' "a form only as part of a longer word"

# --- unread: a file the check cannot read is a finding, not a clean pass
fresh; rm "$F/site/templates/index.html"
judge 10 'site/templates/index.html is missing, so the homepage.s template text is not judged' "a missing template"
fresh; printf 'lead = "unterminated\n' >> "$F/site/data/manifesto.toml"
judge 10 'site/data/manifesto.toml cannot be read as TOML' "a copy file that is not TOML"

# --- real: the checkout's own copy and template pass
fresh
cp "$ROOT/site/data/marketing.toml" "$ROOT/site/data/manifesto.toml" "$ROOT/site/data/homepage.toml" "$F/site/data/"
cp "$ROOT/site/templates/index.html" "$F/site/templates/index.html"
judge 0 '^OK   present .*site/templates/index.html' "the checkout's own copy and template"
grep -q '^FAIL' out.txt && { echo "    the checkout's own files carry a present-state sentence:"; cat out.txt; exit 1; }

# and the hero reads its figures rather than describing them: the plan's issues, the newest
# release record's changes from the changelog, and no milestone progress bar the roadmap
# disowns
TPL="$ROOT/site/templates/index.html"
expect_grep 'load_data\(path="data/generated/changelog.json"\)' "$TPL" || exit 1
expect_grep 'data-product-landings="\{\{ rel.version \}\}"' "$TPL" || exit 1
expect_grep 'data-issues="\{\{ st \}\}"' "$TPL" || exit 1
if grep -qE 'data-product-milestones|data/generated/roadmap.json' "$TPL"; then
  echo "    site/templates/index.html still draws milestone progress from roadmap.json"; exit 1
fi
exit 0
