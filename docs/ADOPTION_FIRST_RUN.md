# First run in a foreign repository — forensic finding

> **Closed.** Re-measured against `master` at `6042f4286` (v0.5.0) the count was **112**,
> not 109: the finding was live and had grown with the rule package. It is now **1**, and
> that one is the adopting repository's own. Section 8 records what was decided, what the
> mechanism actually was — section 3 names it wrongly, and the difference matters — and how
> to re-run the measurement. `docs/ADOPTION.md` no longer promises zero; it states what a
> first run reports.

Majordomus reads healthy from inside its own checkout and broken from outside it. This
document records the measurement, because the difference is 109 failures and nothing in
the repository said so.

Everything below was verified against the published release `v0.3.1` and the tree at
`c2629a26`, by installing the tool the way a stranger does: `curl | sh` from the published
site into a throwaway `HOME`, then a fresh `git init` repository. It was found twice, by
two sessions working independently within minutes of each other, and the two decompositions
agree exactly. Nothing here proposes a change; the reading is a decision, and section 6
states what has to be decided rather than deciding it.

## 0. What this was measured against

A forensic document that cannot be re-run decays into an assertion, so the conditions are
pinned rather than described:

| | |
|---|---|
| tool version | `v0.3.1`, the published release |
| how it was installed | `curl -fsSL .../install.sh \| sh` into a throwaway `HOME`, from the public site — not a local build, not this checkout |
| repository under test | a fresh `git init` with one empty commit and nothing else |
| repository the finding is about | this one, at `c2629a26` |
| rules the package ships | 50, all under `.ai/repo/rules/vendor/majordomus/` |
| rules that produce a finding | 41 |

A later run that differs from this is either drift or a fix, and the numbers above are what
tells them apart.

## 1. The contract this falsifies

`docs/ADOPTION.md` does not merely suggest a first run. It prescribes four commands and
then states an outcome. At the time of this finding it read:

> Add the two hook lines `init` printed. Run `doctor` again; it should report zero
> failures.

That is a promise with a number in it, which is what makes it testable. It no longer says
that: the sentence was removed in favour of a measured description of the second run, which
is section 8's other half.

## 2. What actually happens

The documented path, followed to the letter and with nothing else done to the repository:

```bash
curl -fsSL https://majordomus.dev/install.sh | sh
cd <fresh repository>
majordomus init
majordomus update
# the two hook lines init prints, pasted into .git/hooks/
majordomus doctor
```

`doctor` reports **109 failures** and exits 10. The two wiring checks flip to `OK`, so the
hooks really are satisfied; the 109 are what remains after the document's own instructions
have been carried out.

| count | failure |
|---|---|
| 68 | names claim `<id>`, which is not in `docs/CLAIMS.yaml` |
| 39 | test `test/cases/NN_*.sh` does not exist |
| 1 | `test/run.sh` does not glob `test/cases/`; a new case would not run |
| 1 | no `.github/workflows/validate.yml`; nothing runs the doctrine tests on integration |

Before the hooks are pasted the count is 111: the same 109 plus the two hook lines the
document tells you to add. 109 is the number a person who reads our adoption document and
does what it says will see.

## 3. Why every one of them is unfixable by the adopter

Each of the 109 names something that exists only in this repository: our claims ledger,
our test cases, our test runner, our CI workflow. An adopting repository has none of them
and is not supposed to.

108 of them are evidence paths: a document or a test file that a rule cites as its own
proof. The 109th is a different kind of thing and section 5 takes it separately, so
"rotted evidence path" describes 108 of these findings rather than all of them.

The references come from the rule package the tool installs. For example,
`ai-layout-integrity.v1.md`, as `init` writes it into a foreign repository:

```yaml
x-majordomus:
  claims: [init-refuses, ai-layer-manifest, local-state-ignored, legacy-migration, tool-location-independent]
  tests: [test/cases/01_init.sh]
```

Those two fields describe how *we* verify our own rule. The validator checks them in
whatever repository it runs in, so away from home every one of them resolves to nothing.

This is the whole baseline rather than a noisy corner of it. The 109 come from **41
distinct doctrines** — `ai-layout-integrity` 6, `projection-integrity` 5, then
`state-consistency`, `rule-package-integrity`, `doctrine-wiring-integrity` and
`context-integrity` at 4 each, and a long tail — out of the 50 rules the package ships. In
the fresh repository `.ai/repo/rules/project/` holds only its `README.md`, so no project
rule is involved and nothing is confounded. The finding is exactly that the shipped package
cannot satisfy itself anywhere but at home.

## 4. Why nobody saw it

Every session that has ever run `doctor` ran it inside this checkout, where `test/cases/`
and `docs/CLAIMS.yaml` exist and every reference resolves. The tool is not silent about the
problem; it has simply never been asked the question from outside.

The same shape produced the release defect found the same night: a push made with the
default token starts no workflow, so a release could be published and the site would still
serve a 404 for its metadata. Both are checks that pass in the one environment we look at
and fail in the one the user is in. That is one lesson, not two.

## 5. The one that is wrong twice

Three of the four categories are rotted evidence paths — a document or a test that exists
here and not there. The fourth is not:

```
FAIL doctrine    ci — no .github/workflows/validate.yml; nothing runs the doctrine tests on integration
```

That asserts the adopting repository must carry *Majordomus's own workflow file*. Fixing
the evidence-path problem would silence it, but it would still be a rule about the vendor's
CI enforced against the consumer's repository, which is a separate defect in the same line.
It should be decided separately rather than swept up.

## 6. What has to be decided

The reading both sessions arrived at independently, stated so that it can be accepted or
rejected rather than assumed:

> On a vendored rule, `x-majordomus.tests` and `x-majordomus.claims` are the vendor's
> evidence for its own rule, not an obligation on the repository that adopts it. They
> should be verified in the package's home repository, where the paths resolve, and skipped
> elsewhere.

**The caution that belongs with it.** "Skip when the path does not resolve" is one line
away from a check that cannot fail. Implemented lazily it skips the evidence everywhere,
including here, and a package could then ship rules whose tests and claims had rotted with
nothing to say so. For an adopter the two implementations behave identically, and in a diff
they look identical. What distinguishes them is a test in *this* repository proving the
evidence is still checked here. Without that test the fix is indistinguishable from the
defect it replaces.

**Not decided here:** whether the vendored package should carry those fields at all, or
carry them under a name that says whose evidence they are; whether the CI rule in section 5
should be removed, narrowed to the vendor, or restated as something an adopter can satisfy;
and whether the two hook findings should read as pending rather than failed, which is a
wording question with no design content and should not ride on the larger decision.

## 7. Reproduction

```bash
T=$(mktemp -d); export HOME="$T"
curl -fsSL https://majordomus.dev/install.sh | sh
mkdir "$T/r" && cd "$T/r" && git init -q . && git commit -q --allow-empty -m init
"$T/.local/bin/majordomus" init
"$T/.local/bin/majordomus" update
printf '#!/bin/sh\n%s doctor || exit $?\n' "$T/.local/bin/majordomus" > .git/hooks/pre-commit
printf '#!/bin/sh\n%s finish --check || exit $?\n' "$T/.local/bin/majordomus" > .git/hooks/pre-push
chmod +x .git/hooks/pre-commit .git/hooks/pre-push
"$T/.local/bin/majordomus" doctor; echo "exit $?"     # 109 failures, exit 10
rm -rf "$T"
```

Two cautions for anyone re-running it. `export HOME="$T"` is needed by the installer and
breaks a version-manager shim on the way — an `asdf`-managed `python3` stops resolving, and
`doctor` then reports all 41 schemas as unparseable, which is the harness and not the tool.
And a `MAJORDOMUS_SHARE` inherited from a Majordomus checkout points the installed tool at
that checkout's `share/`; a newcomer has no such variable, so run every command under
`env -u MAJORDOMUS_SHARE`.

## 8. Resolution

Re-measured on `master` at `6042f4286` (v0.5.0), following `ADOPTION.md` literally into a
throwaway `git init` repository holding a README, one source file and one commit:

| | failures | exit | of which the repository's own |
|---|---|---|---|
| before | 112 | 10 | 1 |
| after | 1 | 10 | 1 |
| after, and the one finding fixed | 0 | 0 | — |

The third row is the point: zero was always reachable, and 111 findings stood between the
adopter and it. Fixing the survivor took one sentence in the repository's `README.md`.

The 112 decompose as section 2 describes, one larger in each of the first two rows because
the package now ships more rules: 69 claims, 40 test paths, the runner, the CI workflow,
and one `bootstrap` finding that is genuinely the adopter's — the repository's `README.md`
does not name the `AGENTS.md` that `update` just wrote. That one survives, deliberately: it
is true, it is the adopter's, and one line closes it.

### The mechanism, corrected

Section 3 says the fields are "checked in whatever repository it runs in". That is not what
happens, and the difference is the whole fix. `mj_validate_doctrine_wiring` resolves them
against `MJ_HOME` — the *tool distribution* — never against `MJ_ROOT`. `lib/common.sh` is
explicit about the two roots and has been all along.

The experiment that settles it: run a Majordomus **checkout's** `bin/majordomus doctor`
against the same foreign repository, changing nothing else. 110 becomes 3. The findings
never described the adopting repository; they described whichever tree the tool was run
from, and `scripts/release-package` deliberately ships `bin/ lib/ libexec/ share/ LICENSE`
and a `RELEASE.json` stamp — no `test/`, no `docs/CLAIMS.yaml`, no `.github/`. So the
evidence is unreadable in every installed copy by construction, and an adopting repository
merely happened to be where the resulting noise was read.

This is why "make the check resolve against the repository instead" would have fixed
nothing, and why the count was identical whatever the adopter's repository contained.

### What was decided

The reading in section 6 is **accepted**, with the discriminator sharpened. A vendored
rule's `tests` and `claims` are the vendor's evidence and are verified where the vendor's
source is, but the tool decides that from the `RELEASE.json` stamp `release-package` writes
and `release-verify` refuses an archive without — never from whether the evidence files
happen to be present.

That is precisely the caution section 6 raises. "Skip when the path does not resolve" is
the same diff and a check that can never fail anywhere, including here, and a package could
then ship rotted evidence with nothing to say so. `test/cases/97_vendor_evidence.sh` is
what distinguishes the two: it asserts that an unstamped tree still reads the evidence and
still fails over a missing test path, that a stamped one does not, and that stamping the
same tree is what flips it.

Section 5's CI finding is settled by the same rule rather than separately: it is the
vendor's workflow, verified in the vendor's source and asked of nobody else.

The skip is announced, not silent — `INFO doctrine vendor evidence`, naming the release —
because a reader has to be able to tell it from evidence that was checked and passed. It
follows a precedent already in `doctor`: `INFO command coverage — this installation
carries no test suite to measure`.

`majordomus doctrine status` carried the same defect one command along, counting the same
unreadable test files and exiting 10 in every adopting repository. It is fixed the same
way, and reports `not readable here` rather than a misleading zero.

**Still open, and deliberately not done here.** Two wording questions, neither of which the
promise depends on:

- whether the package should carry those fields under a name that says whose evidence they
  are — a rename with a migration, not a defect;
- whether `doctrine-wiring-integrity.v1`'s statement should say where each clause is
  verified. It reads "…is proved by a test that CI runs", which is true of the vendor's
  rule set and is now checked in the vendor's source. Editing it re-hashes the rule in
  `share/standard/majordomus/manifest.yaml` and re-vendors the copy under
  `.ai/repo/rules/vendor/`, which is more churn than a clarification is worth while the
  behaviour and both documents already say it plainly.

The two hook findings section 6 mentions no longer arise: the four documented commands
leave the hooks wired and both checks green.
