# json_unesc.awk — one JSON string, decoded to the bytes it stands for.
#
# Reads the raw span json_scan.awk emitted for a string member — still quoted, still
# escaped — and writes what it denotes: no trailing newline, no quoting, nothing added.
# This is the inverse of nothing: the record keeps the raw span, and this exists so a
# person can read one. A decode that is only ever a rendering can be lossy in one
# direction and still be honest, but it must never be lossy in silence, so a byte the
# grammar does not allow stops the decode with exit 3 and a reason on stderr rather than
# producing a shorter string.
#
# \u is decoded to UTF-8, surrogate pairs included, one byte at a time through %c rather
# than through the locale, because the byte sequence must not depend on where this runs.
# Run it under LC_ALL=C for the same reason.

function err(m) { printf "ERROR:%s\n", m > "/dev/stderr"; exit 3 }

function hex4(h,   i, c, d, v) {
  if (length(h) != 4) err("a \\u escape needs four hex digits")
  v = 0
  for (i = 1; i <= 4; i++) {
    c = tolower(substr(h, i, 1))
    d = index("0123456789abcdef", c) - 1
    if (d < 0) err("a \\u escape carries a byte that is not a hex digit: " c)
    v = v * 16 + d
  }
  return v
}

# a code point as UTF-8 bytes
function utf8(c) {
  if (c < 128)   return sprintf("%c", c)
  if (c < 2048)  return sprintf("%c%c", 192 + int(c / 64), 128 + c % 64)
  if (c < 65536) return sprintf("%c%c%c", 224 + int(c / 4096), 128 + int(c / 64) % 64, 128 + c % 64)
  return sprintf("%c%c%c%c", 240 + int(c / 262144), 128 + int(c / 4096) % 64,
                             128 + int(c / 64) % 64, 128 + c % 64)
}

{ s = s $0 }

END {
  n = length(s)
  if (n < 2 || substr(s, 1, 1) != "\"" || substr(s, n, 1) != "\"") err("not a quoted string")
  out = ""
  i = 2
  while (i < n) {
    c = substr(s, i, 1)
    if (c != "\\") { out = out c; i++; continue }
    e = substr(s, i + 1, 1)
    if (e == "")  err("a backslash ends the string")
    else if (e == "\"") { out = out "\""; i += 2 }
    else if (e == "\\") { out = out "\\"; i += 2 }
    else if (e == "/")  { out = out "/";  i += 2 }
    else if (e == "b")  { out = out sprintf("%c", 8);  i += 2 }
    else if (e == "f")  { out = out sprintf("%c", 12); i += 2 }
    else if (e == "n")  { out = out "\n"; i += 2 }
    else if (e == "r")  { out = out "\r"; i += 2 }
    else if (e == "t")  { out = out "\t"; i += 2 }
    else if (e == "u") {
      cp = hex4(substr(s, i + 2, 4)); i += 6
      # a high surrogate is half of a code point; the low half must follow it
      if (cp >= 55296 && cp < 56320) {
        if (substr(s, i, 2) != "\\u") err("a high surrogate is not followed by \\u")
        lo = hex4(substr(s, i + 2, 4)); i += 6
        if (lo < 56320 || lo >= 57344) err("a high surrogate is not followed by a low one")
        cp = 65536 + (cp - 55296) * 1024 + (lo - 56320)
      } else if (cp >= 56320 && cp < 57344) err("a low surrogate stands alone")
      out = out utf8(cp)
    }
    else err("a backslash is followed by a byte no escape names: " e)
  }
  printf "%s", out
}
