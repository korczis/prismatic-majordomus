# codeblock.awk — prints the Nth fenced code block of a Markdown file, without fences.
# -v n=1   which block (1-based)
BEGIN { seen = 0; incode = 0 }
/^[ \t]*```/ {
  if (!incode) { seen++; incode = 1; next }
  incode = 0
  if (seen == n) exit 0
  next
}
incode && seen == n { print }
END { if (seen < n) exit 3 }
