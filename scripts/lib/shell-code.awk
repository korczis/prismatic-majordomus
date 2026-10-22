# shell-code.awk — print a shell file with every quoted region blanked out, so that a
# grep-based check can look at the commands a script runs rather than at the text it
# carries.
#
# Why this exists. scripts/ci/order-check counts `sort` invocations that are not pinned
# with LC_ALL=C. It reported thirteen where the truth was two, because jq's `sort` builtin
# spells itself exactly like sort(1) and appears after a pipe exactly like sort(1) — four of
# the thirteen were inside single-quoted jq filters in one behavioural case. Nobody can pin
# jq's sort, so the ratchet could not be satisfied by anyone and failed every branch. The
# same shape has now been found three times in this repository in a single day: a detector
# that matches a literal also matches the prose that explains it, the fixture that
# demonstrates it, and another language that spells it the same way.
#
# What is blanked: text between single quotes, text between double quotes, and the
# character after a backslash. Each becomes a space, so column positions and the length of
# the line are preserved and an offset reported against the output still lines up with the
# input. What survives is command words, operators, and unquoted assignments — including
# `LC_ALL=C`, which is never quoted, so a caller may still grep the output for the pin.
#
# Quote state is carried across lines on purpose: a shell quote may open on one line and
# close on another, and a per-line scanner desynchronises at the first multi-line jq
# program and then mis-reads every line after it.
#
# Comments are blanked too, and that is not a convenience: a comment is the single largest
# source of the very confusion this file exists to remove. A comment is also where an
# apostrophe lives — "the repository's own counts", "the gate's behaviour" — and an
# apostrophe read as an opening quote desynchronises every line after it. That is not
# hypothetical: the first draft of this scanner reported four sort sites in two files, all
# four of them inside single quotes, because line 3 of one case says "repository's" in a
# comment. A scanner that cannot survive an apostrophe is worse than no scanner, because
# its output looks like evidence.
#
# What this does NOT do, stated so a caller does not assume it: it does not track heredocs
# (a heredoc body is emitted as-is), and it does not understand $'...' or ANSI-C quoting
# beyond treating the quote as an ordinary single quote.
#
#   awk -f scripts/lib/shell-code.awk <file>       # blanked lines, one per input line
#   awk -v prefix=path/to/f -f ... <file>          # each line as prefix:lineno:text
BEGIN { state = 0; SQ = sprintf("%c", 39); DQ = sprintf("%c", 34); BS = sprintf("%c", 92) }
{
  raw = $0; out = ""; i = 1; n = length(raw)
  while (i <= n) {
    c = substr(raw, i, 1)
    if (state == 0) {
      # A `#` in command position starts a comment; one inside a word (${x#y}, a URL
      # fragment, a colour literal) does not. The test is what precedes it, which is the
      # POSIX rule and is enough for shell anybody writes.
      if (c == "#" && (i == 1 || substr(raw, i - 1, 1) ~ /[ \t;&|(]/)) {
        while (i <= n) { out = out " "; i++ }
        break
      }
      if (c == SQ) { state = 1; out = out " " }
      else if (c == DQ) { state = 2; out = out " " }
      else if (c == BS) { out = out "  "; i++ }
      else out = out c
    } else if (state == 1) {
      if (c == SQ) state = 0
      out = out " "
    } else {
      if (c == BS) { out = out "  "; i++ }
      else { if (c == DQ) state = 0; out = out " " }
    }
    i++
  }
  if (prefix != "") printf "%s:%d:%s\n", prefix, NR, out
  else print out
}
