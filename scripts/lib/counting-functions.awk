# Print, one per line, the name of every shell function that reports into a variable it did not
# declare `local`: one whose body adds one to it (`fail() { echo "..."; fails=$((fails+1)); }`)
# or sets it to a flag (`bad() { echo "..."; fail=1; }`, as scripts/ci/pages-check reports).
# Called inside a loop that a pipe feeds, such a function counts or flags into a subshell, and
# the verdict is gone when the loop ends, whatever the variable is for. A variable the function
# keeps `local` is its own business. A flag is an assignment in command position (at the start
# of the body or after `;`, `{`, a newline, `&&`, `||`, `then`, `do` or `else`) of 1, true or
# yes; text inside quotes is not read, and `export`, `readonly` and `declare` are not commands
# that set a flag. Definitions are found wherever they sit on a line; a body runs from its
# `name() {` to the brace that closes it, with `${...}` expansions not counted as braces.
# scripts/lib/counting-functions.fixture holds the known answer; shell-lint checks it first.
function braces_in(s,   t, o, c) { t = s; gsub(/\$\{[^}]*\}/, "", t); o = gsub(/\{/, "{", t); c = gsub(/\}/, "}", t); return o - c }
function judge(name, text,   rest, m, v, w, locals, parts, n, k) {
  split("", locals)
  rest = text
  while (match(rest, /(^|[;{ \t\n])local[ \t]+[^;\n]*/)) {
    m = substr(rest, RSTART, RLENGTH); rest = substr(rest, RSTART + RLENGTH)
    sub(/^[;{ \t\n]*local[ \t]+/, "", m); n = split(m, parts, /[ \t]+/)
    for (k = 1; k <= n; k++) { v = parts[k]; sub(/=.*/, "", v); if (v != "") locals[v] = 1 }
  }
  rest = text
  while (match(rest, /[A-Za-z_][A-Za-z0-9_]*=\$\(\( *[A-Za-z_][A-Za-z0-9_]* *\+ *1 *\)\)/)) {
    m = substr(rest, RSTART, RLENGTH); rest = substr(rest, RSTART + RLENGTH)
    v = m; sub(/=.*/, "", v)
    w = m; sub(/^[^(]*\(\( */, "", w); sub(/ *\+.*/, "", w)
    if (v == w && !(v in locals)) { print name; return }
  }
  # a flag: quoted text removed first, so `echo "fail=1"` is words and not an assignment
  rest = text
  gsub(/"[^"]*"/, "\"\"", rest); gsub(/'[^']*'/, "''", rest)
  while (match(rest, /((^|[;{\n]|&&|\|\|)[ \t]*|(^|[ \t;\n])(then|do|else)[ \t]+)[A-Za-z_][A-Za-z0-9_]*=(1|true|yes)([ \t;}\n]|$)/)) {
    m = substr(rest, RSTART, RLENGTH); rest = substr(rest, RSTART + RLENGTH)
    v = m; sub(/=.*/, "", v); sub(/^.*[^A-Za-z0-9_]/, "", v)
    if (!(v in locals)) { print name; return }
  }
}
{
  line = $0
  if (fname != "") {
    body = body "\n" line; depth += braces_in(line)
    if (depth <= 0) { judge(fname, body); fname = "" }
    next
  }
  rest = line
  while (match(rest, /[A-Za-z_][A-Za-z0-9_]*\(\) *\{/)) {
    name = substr(rest, RSTART, RLENGTH); sub(/\(\).*/, "", name)
    after = substr(rest, RSTART + RLENGTH)
    t = after; gsub(/\$\{[^}]*\}/, "", t)
    d = 1; inner = ""; closed = 0
    for (k = 1; k <= length(t); k++) { ch = substr(t, k, 1); if (ch == "{") d++; else if (ch == "}") { d--; if (d == 0) { closed = 1; break } } inner = inner ch }
    if (closed) { judge(name, inner); rest = substr(t, k + 1); continue }
    fname = name; body = after; depth = braces_in("{" after); break
  }
}
