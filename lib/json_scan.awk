# json_scan.awk — the top-level scalar members of one JSON object, raw.
#
# Reads one JSON object and prints a line per member whose value is a scalar: the key, a
# tab, and the value exactly as it appeared in the input — a string still quoted and still
# escaped. Nested objects and arrays are skipped rather than flattened, because the caller
# wants a provider's payload fields and not a document model, and returning the raw span
# means a captured value is byte-identical to what the provider sent: no decode and encode
# round trip that could lose an escape.
#
# A byte the grammar does not allow stops the scan with exit 3 and a reason on stderr. A
# partial reading of a payload is worse than none: the caller must be able to tell a
# payload it understood from one it did not.

function err(m) { printf "ERROR:%s\n", m > "/dev/stderr"; exit 3 }

# index of the first byte at or after i that is not JSON whitespace
function ws(i) { while (i <= n && index(" \t\r\n", substr(s, i, 1)) > 0) i++; return i }

# i is at an opening quote; returns the index one past the closing quote
function str(i,   c) {
  i++
  for (;;) {
    if (i > n) err("a string is not closed")
    c = substr(s, i, 1)
    if (c == "\\") { i += 2; continue }
    if (c == "\"") return i + 1
    i++
  }
}

# i is at '{' or '['; returns the index one past its match, strings respected
function skip(i,   d, c) {
  d = 0
  for (;;) {
    if (i > n) err("a nested value is not closed")
    c = substr(s, i, 1)
    if (c == "\"") { i = str(i); continue }
    if (c == "{" || c == "[") d++
    else if (c == "}" || c == "]") { d--; if (d == 0) return i + 1 }
    i++
  }
}

function emit(k, v) {
  if (index(k, "\t") > 0 || index(v, "\t") > 0) err("a member carries a raw tab, which JSON does not allow in a string")
  printf "%s\t%s\n", k, v
}

{ s = s $0 "\n" }

END {
  n = length(s); i = ws(1)
  if (substr(s, i, 1) != "{") err("the payload does not begin with an object")
  i = ws(i + 1)
  if (substr(s, i, 1) == "}") exit 0
  for (;;) {
    i = ws(i)
    if (substr(s, i, 1) != "\"") err("a member name must be a string, at byte " i)
    st = i; i = str(i); k = substr(s, st + 1, i - st - 2)
    i = ws(i)
    if (substr(s, i, 1) != ":") err("a member name must be followed by a colon, at byte " i)
    i = ws(i + 1)
    c = substr(s, i, 1)
    if (c == "\"") { st = i; i = str(i); emit(k, substr(s, st, i - st)) }
    else if (c == "{" || c == "[") i = skip(i)
    else {
      st = i
      while (i <= n && index(",}] \t\r\n", substr(s, i, 1)) == 0) i++
      if (i == st) err("a member has no value, at byte " i)
      emit(k, substr(s, st, i - st))
    }
    i = ws(i)
    c = substr(s, i, 1)
    if (c == ",") { i = ws(i + 1); continue }
    if (c == "}") exit 0
    err("a member must be followed by a comma or a closing brace, at byte " i)
  }
}
