# swallowed-listings.awk — a remote asked for an answer, the answer written to a file, and the
# failure of the asking discarded: `gh ... > file 2>/dev/null || true`. What reads the file
# next cannot tell "the remote has nothing" from "the remote did not answer", and decides
# from an empty file as if it were the first (rule project.a-failed-read-is-not-an-empty-answer).
#
# Prints `<file>:<line>: <reason>` for the first line of each such command; a command is one
# logical line, its backslash continuations joined. A probe whose answer goes into a variable
# is not this shape: a variable is tested where it is set, and whether its emptiness is read
# as "unknown" is review's question, not one a scanner can answer.
{
  if (cmd == "") start = FNR
  line = $0
  cont = (line ~ /\\$/)
  sub(/\\$/, "", line)
  cmd = cmd line
  if (cont) next
  c = cmd; cmd = ""
  if (c ~ /^[[:space:]]*#/) next
  if (c !~ /(^|[^[:alnum:]_.\/-])(gh|curl)[[:space:]]/) next
  if (c !~ /\|\|[[:space:]]*(true|:)[[:space:]]*(;|\)|\}|#|$)/) next
  t = c
  gsub(/'[^']*'/, "''", t)                          # a jq program may hold a `>` of its own
  gsub(/[0-9&]?>+[[:space:]]*\/dev\/null/, "", t)    # a stream thrown away is not an answer kept
  gsub(/[0-9]?>&[0-9]/, "", t)
  if (t !~ /(^|[^0-9&>])>>?[[:space:]]*[^&[:space:]>]/) next
  printf "%s:%d: a remote's answer is written to a file and the failure of asking is discarded (|| true); an answer that could not be had would be read as an empty one\n", FILENAME, start
}
