# majordomus-covers: none
# The gate that refuses a document naming a file that is not there.
#
# The failure is cheap to make and invisible to everything else: the schemas grew a vendor
# and a kind directory, and every document that named the old flat file kept naming it.
# The site still built, the schemas still validated, the suite still passed, and eight
# documents plus one test fixture pointed readers at files that were not there. The site's
# own link check does not see it — these are paths in prose and in tables, not links — and
# no other gate reads a document as a set of claims about the layout.
#
# What is tested here is the same triple case 58 tests for the home-path gate: that the
# gate is wired into core-check, that this repository passes it, and — the part worth a
# test — that each of its three checks actually fails when the fault is present, and that
# each of its exemptions actually holds. A gate that cannot fail is decoration, and an
# exemption that does not hold turns a gate into a reason to stop writing documents.
. "$ROOT/test/lib.sh"

# ---------------------------------------------------------------- wired, and passing here
expect_grep 'every reference resolves' "$ROOT/scripts/ci/core-check"
expect_exit 0 "$ROOT/scripts/ci/reference-check" "$ROOT"
expect_grep 'reference-check: every reference resolves'

# ---------------------------------------------------------------- a repository that passes
# Everything the gate must not complain about, in one repository: a reference that resolves,
# a path inside a fenced block that the document itself creates, a path the repository
# ignores by design, and a record naming a file that has since moved.
R="$T/repo"
mkdir -p "$R/docs" "$R/.ai/repo/adrs" "$R/share/schemas/majordomus/policy" "$R/site/data/generated"
git -C "$R" init -q
git -C "$R" config user.email t@example.com
git -C "$R" config user.name t
printf 'site/static/\n' > "$R/.gitignore"
printf '# Design\n\nThe layout is in `docs/DESIGN.md`, and the policy schema is\n`share/schemas/majordomus/policy/policy.v1.schema.json`.\n' > "$R/docs/README.md"
printf '# Design\n\nThe build writes `site/static/build.json`, which is not committed.\n' > "$R/docs/DESIGN.md"
cat > "$R/docs/DEMO.md" <<'DOC'
# Demo

Write a file the repository does not carry:

```sh
printf 'x\n' > docs/SCRATCH.md
```

and the tool reports `docs/SCRATCH.md` as unreadable.
DOC
printf -- '---\nid: 1\n---\n\n# A record\n\nThe schema was `share/schemas/policy.schema.json` when this was written.\n' \
  > "$R/.ai/repo/adrs/0001-a-record.md"
printf '{}\n' > "$R/share/schemas/majordomus/policy/policy.v1.schema.json"
printf '{ "objects": [ { "contract": "share/schemas/majordomus/policy/policy.v1.schema.json" } ] }\n' \
  > "$R/site/data/generated/objects.json"
git -C "$R" add -A && git -C "$R" commit -qm seed
expect_exit 0 "$ROOT/scripts/ci/reference-check" "$R"
expect_grep 'every reference resolves'

# ---------------------------------------------------------------- and each check can fail
# 1. a reference in prose that resolves nowhere
printf '\nThe protocol is in `docs/PROTOCOL.md`.\n' >> "$R/docs/README.md"
expect_exit 1 "$ROOT/scripts/ci/reference-check" "$R"
expect_grep 'docs/README.md names docs/PROTOCOL.md'
git -C "$R" checkout -q -- docs/README.md

# 2. a schema named by a path no identity can produce, in a document and in a fixture alike
printf '\nThe policy schema is `share/schemas/policy.schema.json`.\n' >> "$R/docs/README.md"
expect_exit 1 "$ROOT/scripts/ci/reference-check" "$R"
expect_grep 'share/schemas/<vendor>/<name>/<name>.v<n>'
expect_grep 'share/schemas/policy.schema.json'
git -C "$R" checkout -q -- docs/README.md
printf 'p="$(plan share/schemas/policy.schema.json)"\n' > "$R/test-case.sh"
git -C "$R" add test-case.sh
expect_exit 1 "$ROOT/scripts/ci/reference-check" "$R"
git -C "$R" rm -q -f test-case.sh

# 3. a contract published to a reader that is not in the repository
if command -v jq >/dev/null 2>&1; then
  printf '{ "objects": [ { "contract": "share/schemas/majordomus/gone/gone.v1.schema.json" } ] }\n' \
    > "$R/site/data/generated/objects.json"
  expect_exit 1 "$ROOT/scripts/ci/reference-check" "$R"
  expect_grep 'publishes contract share/schemas/majordomus/gone/gone.v1.schema.json'
  git -C "$R" checkout -q -- site/data/generated/objects.json
fi

# ---------------------------------------------------------------- and each exemption holds
# The exemptions are the reason the gate can be strict. Each is proved by making the fault
# and expecting silence, so that a later tightening of the extraction cannot quietly turn
# a document's own demonstration, or a record, into a failure.
#
# a record keeps the path it was written with — checked above, and again here on its own
expect_exit 0 "$ROOT/scripts/ci/reference-check" "$R"
printf '\nAnd `docs/GONE.md` explained it.\n' >> "$R/.ai/repo/adrs/0001-a-record.md"
expect_exit 0 "$ROOT/scripts/ci/reference-check" "$R"
git -C "$R" checkout -q -- .ai/repo/adrs/0001-a-record.md
# a path the repository ignores is not a broken reference
printf '\nThe run is `site/static/run.json`.\n' >> "$R/docs/DESIGN.md"
expect_exit 0 "$ROOT/scripts/ci/reference-check" "$R"
git -C "$R" checkout -q -- docs/DESIGN.md
# a path a document creates in its own fence, then names in prose
printf '\nAnd `docs/SCRATCH.md` is removed afterwards.\n' >> "$R/docs/DEMO.md"
expect_exit 0 "$ROOT/scripts/ci/reference-check" "$R"
git -C "$R" checkout -q -- docs/DEMO.md
# a path that stops at a placeholder is a prefix, not a path
printf '\nEach release is `scripts/release-<version>.sh` and each kind\n`share/schemas/majordomus/<kind>/`.\n' >> "$R/docs/README.md"
expect_exit 0 "$ROOT/scripts/ci/reference-check" "$R"
git -C "$R" checkout -q -- docs/README.md

# ---------------------------------------------------------------- and it reads the index
# The reference must resolve in a fresh clone, so a file present but untracked satisfies
# nothing. This is what makes the gate meaningful in CI, where nobody's scratch file exists.
printf '# Protocol\n' > "$R/docs/PROTOCOL.md"
printf '\nThe protocol is in `docs/PROTOCOL.md`.\n' >> "$R/docs/README.md"
expect_exit 1 "$ROOT/scripts/ci/reference-check" "$R"
expect_grep 'docs/README.md names docs/PROTOCOL.md'
git -C "$R" add docs/PROTOCOL.md
expect_exit 0 "$ROOT/scripts/ci/reference-check" "$R"
