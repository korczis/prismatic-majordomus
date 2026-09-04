# Start here

What to read before changing this repository, in order:

1. `docs/DESIGN.md` — what the tool is and why, the models, the boundaries, and what is
   intentionally absent.
2. `docs/CLI.md` — every command: behaviour, reads, writes, exit-code contract.
3. `docs/SCHEMAS.md` — every file the tool reads or writes, with an example each.
4. `docs/EXTRACTION_REPORT.md` — the evidence behind each decision, and what was rejected.
5. `docs/DOCTRINE.md` — what is enforced, by what, and how the wiring is verified.

`docs/CLAIMS.yaml` is the shortest honest answer to what the tool does today. The rules
this repository holds itself to are under `../../rules/project/`; the ones every
Majordomus-supervised repository holds to are under `../../rules/vendor/majordomus/`.
