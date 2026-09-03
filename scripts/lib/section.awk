# section.awk — prints the body of one Markdown section.
#
# -v want="The problem"   heading text to match, exactly, after the # marks
# -v level=2              heading level to match (0 = any)
# -v withheading=1        include the heading line itself
#
# The section ends at the next heading of the same or a higher level. Exits non-zero
# through the `found` flag so the caller can fail loudly when a canonical heading is
# renamed upstream, rather than silently emitting an empty section.
BEGIN { found = 0; inside = 0 }
/^#{1,6} / {
  match($0, /^#+/); lvl = RLENGTH
  txt = substr($0, lvl + 2); sub(/[ \t]+$/, "", txt)
  if (inside && lvl <= mylvl) { inside = 0 }
  if (!found && txt == want && (level == 0 || lvl == level)) {
    found = 1; inside = 1; mylvl = lvl
    if (withheading == 1) print
    next
  }
  if (inside) print
  next
}
inside { print }
END { if (!found) exit 3 }
