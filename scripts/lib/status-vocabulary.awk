# status-vocabulary.awk — holds the built site to one status vocabulary, for scripts/site-check.
#
#   awk -v mode=badges -v vocab=<file> -f status-vocabulary.awk <page.html>...
#     Every badge partials/status-badge.html rendered carries `data-status-kind` and
#     `data-status`; each is judged against the vocabulary of its kind, read from <file>:
#       claim<TAB><word>             a status docs/CLAIMS.yaml declares
#       feature<TAB><word>           a status feature/v1 allows
#       proof<TAB><claim><TAB><word> the state `majordomus evidence show` derived for a claim,
#                                    present only when the evidence reached the build
#     A claim or feature badge whose word its vocabulary does not hold fails; a proof badge
#     must name its claim and say exactly the derived state, or `unknown` where none was
#     derived; a kind outside the three fails.
#
#   awk -v mode=refusals -v want=<n> -f status-vocabulary.awk <page.html>
#     The page's words may state how often the recorded run refused only as the run counted
#     it: a count in digits or in words before "refusal(s)", or after "refuse(s|d)" as
#     once, twice, thrice or "<n> times", must equal <n>. Text and attribute values (a meta
#     description is read too) are both searched.
#
# Output: one "FAIL<TAB><file><TAB><reason>" line per finding, then "COUNT<TAB><n>" with the
# number of badges (mode badges) or counts (mode refusals) examined. Exit 0; the caller
# decides. POSIX awk: no gawk extension is used, so the BSD awk of macOS runs it as CI does.
function attr(tag, name,   s) {
  if (!match(tag, name "=\"[^\"]*\"")) return "\001"
  s = substr(tag, RSTART + length(name) + 2, RLENGTH - length(name) - 3)
  return s
}
function num(w) {
  if (w ~ /^[0-9]+$/) return w + 0
  return (w in WORDS) ? WORDS[w] : -1
}
function judge_count(file, n, phrase) {
  counted++
  if (n != want + 0) printf "FAIL\t%s\tthe page says \"%s\", and the recorded run counts %d refusal(s)\n", file, phrase, want
}
function scan_refusals(file, text,   rest, p, w, phrase) {
  # padded, so a count at either end of the text has a non-word character beside it
  text = " " tolower(text) " "
  gsub(/[ \t]+/, " ", text)
  rest = text
  while (match(rest, /[^a-z0-9]([0-9]+|one|two|three|four|five|six|seven|eight|nine|ten) refusals?[^a-z]/)) {
    phrase = substr(rest, RSTART, RLENGTH)
    p = substr(phrase, 2, length(phrase) - 2); w = p; sub(/ .*$/, "", w)
    judge_count(file, num(w), p)
    rest = substr(rest, RSTART + RLENGTH)
  }
  rest = text
  while (match(rest, /refus(e|es|ed) ((it|them|the work) )?(once|twice|thrice|([0-9]+|one|two|three|four|five|six|seven|eight|nine|ten) times)/)) {
    phrase = substr(rest, RSTART, RLENGTH)
    w = phrase; sub(/^refus(e|es|ed) ((it|them|the work) )?/, "", w); sub(/ times$/, "", w)
    judge_count(file, num(w), phrase)
    rest = substr(rest, RSTART + RLENGTH)
  }
}
BEGIN {
  FS = "\t"; counted = 0; examined = 0
  split("one two three four five six seven eight nine ten", ws, " ")
  for (i = 1; i <= 10; i++) WORDS[ws[i]] = i
  WORDS["once"] = 1; WORDS["twice"] = 2; WORDS["thrice"] = 3
  if (mode == "badges") {
    if (vocab == "") { print "FAIL\t-\tno vocabulary file was given"; exit 0 }
    while ((getline line < vocab) > 0) {
      n = split(line, f, "\t")
      if (f[1] == "claim" || f[1] == "feature") known[f[1] SUBSEP f[2]] = 1
      else if (f[1] == "proof" && n >= 3) proof[f[2]] = f[3]
    }
    close(vocab)
  } else if (mode != "refusals") { print "FAIL\t-\tunknown mode '" mode "'"; exit 0 }
}
mode == "badges" {
  rest = $0
  while (match(rest, /<[a-z]+ [^>]*data-status-kind="[^"]*"[^>]*>/)) {
    tag = substr(rest, RSTART, RLENGTH); rest = substr(rest, RSTART + RLENGTH)
    examined++
    kind = attr(tag, "data-status-kind"); word = attr(tag, "data-status")
    if (word == "\001") { printf "FAIL\t%s\ta %s badge carries no data-status\n", FILENAME, kind; continue }
    if (kind == "claim" || kind == "feature") {
      if (!((kind SUBSEP word) in known))
        printf "FAIL\t%s\ta %s badge says '%s', which the %s vocabulary does not hold\n", FILENAME, kind, word, kind
    } else if (kind == "proof") {
      claim = attr(tag, "data-claim")
      if (claim == "\001" || claim == "") { printf "FAIL\t%s\ta proof badge names no claim\n", FILENAME; continue }
      expect = (claim in proof) ? proof[claim] : "unknown"
      if (word != expect)
        printf "FAIL\t%s\tthe proof badge of claim '%s' says '%s', and the evidence derived '%s'\n", FILENAME, claim, word, expect
    } else {
      printf "FAIL\t%s\ta badge declares the status kind '%s'; the kinds are claim, proof and feature\n", FILENAME, kind
    }
  }
  next
}
mode == "refusals" {
  line = $0; text = ""
  rest = line
  while (match(rest, /content="[^"]*"/)) { text = text " " substr(rest, RSTART + 9, RLENGTH - 10); rest = substr(rest, RSTART + RLENGTH) }
  gsub(/<[^>]*>/, " ", line)
  scan_refusals(FILENAME, line " " text)
}
END { printf "COUNT\t%d\n", (mode == "badges") ? examined : counted }
