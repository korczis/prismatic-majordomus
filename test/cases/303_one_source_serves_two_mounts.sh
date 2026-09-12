# majordomus-covers: none
# The gate of base-path independence, driven against fixture repositories.
#
# One Zola source serves two mounts: the published site at its own origin, and the running
# executable's /docs. That only works while no link in the source states an origin-absolute
# path of its own — `href="/rules/"` is correct on the published site and a 404 under /docs.
# `scripts/site-basepath-check` refuses that literal at the moment it is written, rather
# than after a build nobody ran.
#
# Until this case the gate was named by nothing in the suite. That is worse here than for
# most gates, because this one is a set of regular expressions over markup: the difference
# between a gate that catches origin-absolute links and one that catches *nothing* is a
# character in a pattern, and both look identical on a tree that has no violations.
#
# So every form the gate refuses is written into a fixture, and — just as important — every
# form it must not refuse: a relative link, an anchor, a full URL to somewhere else, a
# protocol-relative URL, a value resolved through Zola, and a loopback URL *quoted in prose*
# rather than offered as a link. The last is the one that would make people route around the
# gate: the MCP documentation prints the server's own log line, and a gate that called that
# a broken link would be deleted within a week.
#
# The gate takes its root from its own location rather than from MJ_ROOT, so the fixture
# carries a copy of it. That is deliberate: what is under test is the script this repository
# ships, read from this checkout, run against a tree that is not this checkout.
. "$ROOT/test/lib.sh"
SRC="$ROOT/scripts/site-basepath-check"
[ -x "$SRC" ] || { echo "    $SRC is not executable"; exit 1; }

# A repository with the shape the gate reads: a committed base_url that is an origin, a
# template and a page whose every link resolves against whatever base the build was given,
# and a script that fetches relative to the page.
fixture() {   # fixture <n> -> prints the tree
  F="$T/tree$1"
  rm -rf "$F"
  mkdir -p "$F/scripts" "$F/site/templates" "$F/site/content"
  cp "$SRC" "$F/scripts/site-basepath-check"
  chmod +x "$F/scripts/site-basepath-check"
  printf 'base_url = "https://majordomus.dev"\ntitle = "Majordomus"\n' > "$F/site/config.toml"
  cat > "$F/site/templates/base.html" <<'HTML'
<a href="{{ get_url(path="rules/") }}">Rules</a>
<a href="../up/">Up</a>
<a href="#top">Top</a>
<a href="https://example.com/x">Away</a>
<a href="//cdn.example.com/x">Protocol relative</a>
<img src="{{ get_url(path="logo.svg") }}" alt="">
HTML
  cat > "$F/site/content/index.md" <<'MD'
---
title: Index
---

See [the rules](../rules/) and [somewhere else](https://example.com/x).
MD
  printf 'export const load = () => fetch("./data/x.json");\n' > "$F/site/app.js"
  ( cd "$F" && git init -q . && git add -A \
      && git -c user.email=t@example.com -c user.name=t commit -q -m fixture )
  printf '%s' "$F"
}

# The gate reads the tracked source, so a fixture that adds a file stages it before running.
gate() {   # gate <tree> [args...]
  local f="$1"; shift
  ( cd "$f" && git add -A >/dev/null 2>&1; cd "$f" && ./scripts/site-basepath-check "$@" )
}

# ---------------------------------------------------------------- the source it accepts
# Six link forms that all resolve against the build's base, in one tree, accepted together.
# This is the half a reader should doubt hardest: a gate that refuses `href="/x"` by also
# refusing `href="../x"` has not made the source portable, it has made it unwritable.
F="$(fixture 0)"
expect_exit 0 gate "$F"
expect_grep 'the source is base-path independent \(base_url https://majordomus.dev\)'

# ---------------------------------------------------------------- 1. the markup form
# The violation: a link written from the origin. Correct on the published site, a 404 under
# the executable's /docs mount, and indistinguishable from the correct form by eye.
F="$(fixture 1)"
printf '<a href="/rules/">Rules</a>\n' >> "$F/site/templates/base.html"
expect_exit 10 gate "$F"
expect_grep 'site/templates/base.html states an origin-absolute link'
expect_grep '<a href="/rules/">Rules</a>'
expect_grep 'Resolve links through Zola \(get_url\) or write them relative, never from the origin'
expect_grep 'The rule is project.web-surface-declared-once'

# restored: the same link resolved through Zola is accepted, so the finding is about the
# literal and not about the target
printf '<a href="{{ get_url(path="rules/") }}">Rules</a>\n' > "$F/site/templates/base.html"
expect_exit 0 gate "$F"

# src is refused as well as href: an image that loads from the origin is as broken under the
# second mount as a link that navigates to it
F="$(fixture 2)"
printf '<img src="/logo.svg" alt="">\n' >> "$F/site/templates/base.html"
expect_exit 10 gate "$F"
expect_grep 'site/templates/base.html states an origin-absolute link'

# and a protocol-relative URL is not an origin-absolute path, however similar it looks
F="$(fixture 3)"
printf '<script src="//cdn.example.com/lib.js"></script>\n' >> "$F/site/templates/base.html"
expect_exit 0 gate "$F"

# ---------------------------------------------------------------- 2. the Markdown form
# The same defect written in the language most of the documentation is written in.
F="$(fixture 4)"
printf '\nSee [the rules](/rules/).\n' >> "$F/site/content/index.md"
expect_exit 10 gate "$F"
expect_grep 'site/content/index.md states an origin-absolute Markdown link'
expect_grep 'See \[the rules\]\(/rules/\)'

printf -- '---\ntitle: Index\n---\n\nSee [the rules](../rules/).\n' > "$F/site/content/index.md"
expect_exit 0 gate "$F"

# ---------------------------------------------------------------- 3. the script form
# A page's script that fetches from the site root is the same defect one layer down, and
# the one nobody sees in review because it is not a link.
F="$(fixture 5)"
printf 'fetch("/data/registry.json");\n' >> "$F/site/app.js"
expect_exit 10 gate "$F"
expect_grep 'site/app.js fetches from an origin-absolute path'

printf 'export const load = () => fetch("./data/registry.json");\n' > "$F/site/app.js"
expect_exit 0 gate "$F"

# ---------------------------------------------------------------- 4. a loopback origin
# Correct for whoever wrote it, wrong for everybody the site is published to.
F="$(fixture 6)"
printf '<a href="http://127.0.0.1:8080/swagger">Swagger</a>\n' >> "$F/site/templates/base.html"
expect_exit 10 gate "$F"
expect_grep 'site/templates/base.html links to a loopback origin'

# and the exemption that keeps the gate usable: a loopback URL a page *quotes* is describing
# a URL, not offering one. The MCP documentation prints the server's own log line, and a
# gate that refused that would be routed around rather than obeyed.
F="$(fixture 7)"
printf '\nThe shared server logs `http://127.0.0.1:56042/` on stderr when it starts.\n' \
  >> "$F/site/content/index.md"
expect_exit 0 gate "$F"

# ---------------------------------------------------------------- 5. the committed base_url
# The committed configuration is the published one; any other mount is a --base-url
# override at build time. A committed base_url that is not an origin publishes a site whose
# every resolved link is wrong, and resolves them so consistently that nothing else notices.
F="$(fixture 8)"
printf 'base_url = "/docs"\ntitle = "Majordomus"\n' > "$F/site/config.toml"
expect_exit 10 gate "$F"
expect_grep "site/config.toml base_url is '/docs'"
expect_grep 'the committed configuration must carry the published origin'

# ---------------------------------------------------------------- 6. a build, when there is one
# What the source cannot show: a base path baked into the output by the build that made it.
# A build serves exactly one mount, and publishing the wrong one is a silent 404 everywhere.
F="$(fixture 9)"
mkdir -p "$F/site/public"
printf '<html><a href="https://majordomus.dev/x">x</a><a href="http://localhost:1111/y">y</a></html>\n' \
  > "$F/site/public/index.html"
expect_exit 10 gate "$F"
expect_grep 'the build at site/public links to a loopback origin'

F="$(fixture 10)"
mkdir -p "$F/site/public"
printf '<html><a href="https://elsewhere.example/x">x</a></html>\n' > "$F/site/public/index.html"
expect_exit 10 gate "$F"
expect_grep 'the build at site/public carries neither the published base nor a /docs mount'
expect_grep 'a build serves exactly one base path'

# both mounts are accepted, because both are legitimate builds of the same source
F="$(fixture 11)"
mkdir -p "$F/site/public"
printf '<html><a href="/docs/rules/">x</a></html>\n' > "$F/site/public/index.html"
expect_exit 0 gate "$F"

F="$(fixture 12)"
mkdir -p "$F/site/public"
printf '<html><a href="https://majordomus.dev/rules/">x</a></html>\n' > "$F/site/public/index.html"
expect_exit 0 gate "$F"

# ---------------------------------------------------------------- the denominator
# Tracked files only, which the gate says and which is load-bearing: site/static/js holds
# third-party bundles copied in at build time, and somebody else's minified library is not
# ours to lint. The consequence is stated here rather than left to be discovered — an
# uncommitted source file is not measured, so the gate's verdict is about the commit.
F="$(fixture 13)"
printf '<a href="/rules/">Rules</a>\n' > "$F/site/templates/rogue.html"
( cd "$F" && ./scripts/site-basepath-check ) >/dev/null 2>&1 && untracked_rc=0 || untracked_rc=$?
[ "$untracked_rc" = 0 ] || {
  echo "    an untracked source file was measured; the gate reads the tracked source"; exit 1; }
# and the moment it is committed, it is a finding
expect_exit 10 gate "$F"
expect_grep 'site/templates/rogue.html states an origin-absolute link'

# ---------------------------------------------------------------- an unusable tree
F="$(fixture 14)"
rm -f "$F/site/config.toml"
expect_exit 12 gate "$F"
expect_grep 'site/config.toml is missing'

expect_exit 2 gate "$F" --deep
expect_grep 'unknown option --deep'

echo "    one source serves two mounts: every origin-absolute form refused, every portable form kept"
