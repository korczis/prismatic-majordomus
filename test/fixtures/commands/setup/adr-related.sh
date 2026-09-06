# One decision, committed, that names what it put in force. `propose` writes provenance —
# where the record came from — and never `related`, which is the other direction and a
# person's statement: this rule, this file, are what the decision did.
. "$FIXTURE_SETUP/adr-recorded.sh"
adr="$(ls .ai/repo/adrs/0001-*.md)"
sed -i.bak 's|^status: proposed$|status: proposed\
related:\
  - rule:majordomus.adr-integrity\
  - file:docs/d|' "$adr" && rm -f "$adr.bak"
git add . && git commit -qm "adr: name the rule and the file the decision put in force"
