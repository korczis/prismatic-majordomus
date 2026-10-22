# Print, one per line, the name of every shell function whose body adds one to a variable it
# did not declare `local` (`fail() { echo "..."; fails=$((fails+1)); }`). Called inside a loop
# that a pipe feeds, such a function counts into a subshell and the count is gone when the loop
# ends, whatever the variable is for. A counter the function keeps `local` is its own business.
# Definitions are found wherever they sit on a line; a body runs from its `name() {` to the
# brace that closes it, with `${...}` expansions not counted as braces.
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
