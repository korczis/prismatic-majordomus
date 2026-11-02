# majordomus-covers: script:scripts/ci/link-check
# claim: a-diagram-is-named-linked-and-addressable
# Every diagram of the built site is named, described and linked by the build, and a link to one
# node of one diagram is decided offline (ADR 0152, issue I2653). The pass is
# scripts/lib/diagrams.mjs, run by scripts/site-build after Zola; the viewer that draws what it
# prepared is site/diagrams.js, held to its behaviour in a browser by the `diagram` spec of
# scripts/interaction-probe.
#
# What is proved, against a site this case builds so that every answer is known beforehand:
#
#   - the pass's own unit tests hold (what a source declares, what a node resolves to);
#   - over a fixture site, each <pre class="mermaid"> gets its id from the heading it sits under
#     or from the name its template declared, the list of its nodes, a place in the tab order and
#     the viewer's toolbar, hidden, right after it;
#   - a node that is the identity of an object the fixture has a page for is linked to that page
#     under the base the site was built for, and one whose page was not built is not linked;
#   - the diagram's source is byte for byte what it was, and a second run changes no file;
#   - two diagrams declared under one name on one page refuse the build (exit 10);
#   - scripts/ci/link-check passes the result, resolving `#diagram-<name>` and
#     `#diagram-<name>:<node>`, and fails a link to a node the diagram does not declare, and a
#     link to a diagram the page does not carry, each with a finding that names it.
. "$ROOT/test/lib.sh"
mj_runs node || skip "$(mj_runs_why node)"
mj_runs python3 || skip "$(mj_runs_why python3)"
PASS="$ROOT/scripts/lib/diagrams.mjs"
CHECK="$ROOT/scripts/ci/link-check"

node --test "$ROOT/scripts/lib/diagrams.test.mjs" > unit.txt 2>&1 || { echo "    the pass's unit tests fail"; grep -E '^not ok|error:' unit.txt | head -20; exit 1; }

fresh() {
  F="$PWD/site"; rm -rf "$F"; mkdir -p "$F/site/public/docs/flow" "$F/site/public/plan/i0001" "$F/site/public/commands/doctor" "$F/site/data/generated" "$F/docs/generated"
  printf 'base_url = "https://example.test"\n' > "$F/site/config.toml"
  printf '{"repository_url": "https://github.com/owner/fixture"}\n' > "$F/site/data/generated/project.json"
  printf '%s\n' '{"surfaces":[{"id":"app","kind":"static-directory","mount":"/","artifact":"site/public","availability":"published-only"}]}' > "$F/docs/generated/web.json"
  printf '%s\n' '{"issues":[{"id":"I0001","title":"Audit"},{"id":"I0002","title":"Unbuilt"}],"milestones":[]}' > "$F/site/data/generated/plan.json"
  printf '%s\n' '{"commands":[{"name":"doctor","summary":"Check the layer"}]}' > "$F/site/data/generated/commands.json"
  git -C "$F" init -q -b master && git -C "$F" add -A && git -C "$F" -c user.name=t -c user.email=t@t commit -qm first
  printf '<!doctype html><html><body><h1 id="audit">I0001</h1><a href="https://example.test/">home</a></body></html>' > "$F/site/public/plan/i0001/index.html"
  printf '<!doctype html><html><body><h1 id="doctor">doctor</h1><a href="https://example.test/">home</a></body></html>' > "$F/site/public/commands/doctor/index.html"
  cat > "$F/site/public/docs/flow/index.html" <<'HTML'
<!doctype html><html><body><h1 id="flow">Flow</h1><h2 id="how-it-flows">How it flows</h2>
<pre class="mermaid">flowchart LR
    A[majordomus doctor] --&gt; I0001
    I0001 --&gt;|then| I0002["I0002&lt;br/&gt;Not built"]
</pre>
<pre class="mermaid">stateDiagram-v2
    [*] --&gt; active
    active --&gt; done
</pre>
<figure><pre class="mermaid m-0" data-diagram="Plan" tabindex="0">flowchart TD
    I0001 --&gt; Z[Zola]
</pre></figure>
<a href="https://example.test/">home</a></body></html>
HTML
  cat > "$F/site/public/index.html" <<'HTML'
<!doctype html><html><body><a href="https://example.test/docs/flow/#diagram-how-it-flows">the flow</a>
<a href="https://example.test/docs/flow/#diagram-how-it-flows:I0001">one node of it</a>
<a href="/docs/flow/#diagram-plan:Z">a node of the plan</a>
<a href="https://example.test/docs/flow/#diagram-how-it-flows-2:active">a state</a></body></html>
HTML
  printf '<?xml version="1.0"?><urlset><url><loc>https://example.test/</loc></url></urlset>' > "$F/site/public/sitemap.xml"
}
pass() { rc=0; node "$PASS" "$F/site/public" "$F/site/data/generated" "https://example.test" > pass.txt 2>&1 || rc=$?; }
check() { rc=0; MJ_ROOT="$F" "$CHECK" > out.txt 2>&1 || rc=$?; }
P() { printf '%s' "$F/site/public/docs/flow/index.html"; }

fresh
cp "$(P)" before.html
pass; [ "$rc" = 0 ] || { echo "    the pass failed on a clean site (exit $rc)"; cat pass.txt; exit 1; }
grep -q '3 diagram(s) named on 1 page(s), 3 node link(s) inferred' pass.txt || { echo "    the pass did not report what it did"; cat pass.txt; exit 1; }

# named: by the heading above, a second one under the same heading, and by the declared name
grep -q '<pre class="mermaid" tabindex="0" id="diagram-how-it-flows" data-diagram="how-it-flows" data-nodes="A I0001 I0002">' "$(P)" || { echo "    the first diagram is not named after its heading, or its nodes are not listed"; exit 1; }
grep -q 'id="diagram-how-it-flows-2" data-diagram="how-it-flows-2" data-nodes="active done">' "$(P)" || { echo "    the second diagram under one heading is not <name>-2"; exit 1; }
grep -q '<pre class="mermaid m-0" tabindex="0" id="diagram-plan" data-diagram="plan" data-nodes="I0001 Z">' "$(P)" || { echo "    a declared name is not the diagram's id"; exit 1; }
# every diagram has the toolbar, hidden, right after it
[ "$(grep -o '<div class="mj-diagram-bar" role="toolbar"[^>]* hidden>' "$(P)" | wc -l | tr -d ' ')" = 3 ] || { echo "    not every diagram has a hidden toolbar"; exit 1; }
[ "$(grep -o 'data-diagram-action="[a-z-]*"' "$(P)" | sort -u | wc -l | tr -d ' ')" = 6 ] || { echo "    the toolbar does not carry its six actions"; exit 1; }
# linked: what has a page, under the base; not what has none
grep -q '<a href="https://example.test/commands/doctor/" data-node="A" data-kind="command">majordomus doctor</a>' "$(P)" || { echo "    the command node is not linked to its page"; exit 1; }
grep -q '<a href="https://example.test/plan/i0001/" data-node="I0001" data-kind="issue">I0001</a>' "$(P)" || { echo "    the issue node is not linked to its page"; exit 1; }
if grep -q 'data-node="I0002"' "$(P)"; then echo "    an issue whose page was not built is linked"; exit 1; fi
if grep -q 'data-node="Z"' "$(P)"; then echo "    a node that names nothing is linked"; exit 1; fi

# the sources are untouched, and a second run changes nothing
python3 - before.html "$(P)" <<'PY3'
import re, sys
body = lambda p: re.findall(r'<pre\b[^>]*>(.*?)</pre>', open(p).read(), re.S)
a, b = body(sys.argv[1]), body(sys.argv[2])
if a != b or len(a) != 3: sys.exit('    the pass changed a diagram\'s source')
PY3
cp "$(P)" once.html
pass; [ "$rc" = 0 ] || { echo "    the second run failed"; cat pass.txt; exit 1; }
cmp -s once.html "$(P)" || { echo "    a second run changed the page"; exit 1; }

# every link the pass wrote, and every diagram and node address, resolves
check; [ "$rc" = 0 ] || { echo "    link-check refuses the annotated site (exit $rc)"; cat out.txt; exit 1; }

# a node the diagram does not declare
sed -i.bak 's|#diagram-plan:Z|#diagram-plan:Nowhere|' "$F/site/public/index.html"
check; [ "$rc" = 10 ] || { echo "    a link to an undeclared node passes (exit $rc)"; cat out.txt; exit 1; }
grep -q 'names node Nowhere of #diagram-plan, which that diagram on /docs/flow/ does not declare' out.txt || { echo "    the finding does not name the node and the diagram"; cat out.txt; exit 1; }
# a node of another diagram on the same page is still not this diagram's
sed -i.bak 's|#diagram-plan:Nowhere|#diagram-plan:active|' "$F/site/public/index.html"
check; [ "$rc" = 10 ] || { echo "    a node of another diagram resolves on this one (exit $rc)"; cat out.txt; exit 1; }
# a diagram the page does not carry
sed -i.bak 's|#diagram-plan:active|#diagram-absent:Z|' "$F/site/public/index.html"
check; [ "$rc" = 10 ] || { echo "    a link into an absent diagram passes (exit $rc)"; cat out.txt; exit 1; }
grep -q 'names #diagram-absent:Z, which /docs/flow/ does not carry' out.txt || { echo "    the finding does not name the absent diagram"; cat out.txt; exit 1; }

# two diagrams declared under one name are one address for two things: refused, and named
fresh
python3 - "$(P)" <<'PY3'
import sys
p = sys.argv[1]; s = open(p).read()
open(p, 'w').write(s.replace('<pre class="mermaid">stateDiagram-v2', '<pre class="mermaid" data-diagram="plan">stateDiagram-v2', 1))
PY3
pass; [ "$rc" = 10 ] || { echo "    two diagrams named plan on one page are accepted (exit $rc)"; cat pass.txt; exit 1; }
grep -q 'docs/flow/index.html: two diagrams on one page are named "plan"' pass.txt || { echo "    the refusal does not name the page and the name"; cat pass.txt; exit 1; }
