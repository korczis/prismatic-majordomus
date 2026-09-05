# Review generated files and documents for drift

Review whether every generated surface matches its canonical source right now, by
running the repository's own drift checks and reading what they do not cover.

```text
Apply the repo-review skill to the generated surfaces: run scripts/generate-site-data
--check and majordomus generate --check, then read the documents under docs/ that
describe behaviour and compare them with the commands they describe. Report every
claim a document makes that no check or test would catch if it became false.
```
