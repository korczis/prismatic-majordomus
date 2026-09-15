# Report a call to a failure-counting function made inside the body of a loop that is the right
# side of a pipe: `... | while ...; do ... fail ...; done`. Bash runs that loop in a subshell, so
# the count the call increments, and any flag it sets, is gone when the loop ends: the finding is
# printed and the gate still exits 0. FNS is a regex of the script's counting functions.
#
# The script is read the way the shell reads it, one character at a time with a stack of
# contexts: a single quote, a double quote, and a command substitution, which inside a double
# quote is parsed as a command again (`"$(sed 's/"/x/')"` is one word, not three). Words are
# only taken at the top level; comments and here-documents are not words. A pipe followed by
# `while` opens a loop, `do` deepens it, `done` closes it, and a call is reported only while a
# loop is open, so `done | head -1 && fail ...` is correctly the main shell's.
function flush(   w) {
  w = word; word = ""
  if (w == "") return
  if (w == "while" && piped) { piped = 0; if (!open) { open = FNR; depth = 0 }; return }
  piped = 0
  if (!open) return
  if (w == "do") { depth++; return }
  if (w == "done") { depth--; if (depth <= 0) { open = 0; depth = 0 }; return }
  if (depth > 0 && w ~ ("^(" FNS ")$")) print FILENAME ":" FNR ": " w " inside the loop piped into at line " open
}
function heredoc(line, i,   j, rest) {
  # `<<EOF` or `<<'EOF'` or `<<-EOF`: the delimiter is read from the line itself, a quoted one is
  # still its name, and the body that follows is text, not shell
  j = i + 2; if (substr(line, j, 1) == "-") j++
  while (substr(line, j, 1) == " " || substr(line, j, 1) == "\t") j++
  rest = substr(line, j)
  if (match(rest, /^[^ \t;|&)<>]+/)) { delim = substr(rest, RSTART, RLENGTH); gsub(/['"\\]/, "", delim); heredoc_next = 1; return j + RLENGTH - 1 }
  return i + 1
}
function push(ctx) { stack[++sp] = ctx }
function pop() { if (sp > 0) sp-- }
{
  if (in_heredoc) { t = $0; sub(/^\t+/, "", t); if (t == delim) in_heredoc = 0; next }
  line = $0; n = length(line)
  for (i = 1; i <= n; i++) {
    c = substr(line, i, 1); c2 = substr(line, i, 2)
    top = (sp > 0) ? stack[sp] : ""
    if (top == "'") { if (c == "'") pop(); continue }
    if (top == "\"") {
      if (c == "\\") { i++; continue }
      if (c == "\"") { pop(); continue }
      if (c2 == "$(") { push("("); i++; continue }
      if (c == "`") { push("`"); continue }
      continue
    }
    if (top == "(" || top == "`") {
      if (c == "\\") { i++; continue }
      if (c == "'") { push("'"); continue }
      if (c == "\"") { push("\""); continue }
      if (top == "`" && c == "`") { pop(); continue }
      if (c2 == "<<" && substr(line, i, 3) != "<<<") { i = heredoc(line, i); continue }
      if (top == "(" && c == "(") { push("("); continue }
      if (top == "(" && c == ")") { pop(); continue }
      if (c == "#" && (i == 1 || substr(line, i - 1, 1) ~ /[ \t;]/)) break
      continue
    }
    # the top level: words
    if (c == "\\") { word = word "Q"; i++; continue }
    if (c == "'" || c == "\"") { push(c); word = word "Q"; continue }
    if (c2 == "$(") { push("("); word = word "Q"; i++; continue }
    if (c == "`") { push("`"); word = word "Q"; continue }
    if (c == "#" && word == "") { break }
    if (c2 == "<<" && substr(line, i, 3) != "<<<") { flush(); i = heredoc(line, i); continue }
    if (c == "|") { if (c2 == "||") { flush(); piped = 0; i++; continue } flush(); piped = 1; continue }
    if (c == " " || c == "\t") { flush(); continue }
    if (c == ";" || c == "&" || c == "(" || c == ")" || c == "{" || c == "}") { flush(); piped = 0; continue }
    word = word c
  }
  if (sp == 0) flush()
  if (heredoc_next) { in_heredoc = 1; heredoc_next = 0 }
}
END { if (sp > 0) { print FILENAME ": unterminated " stack[sp] " at end of file; the scan is not trustworthy"; exit 2 } }
