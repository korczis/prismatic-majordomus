+++
title = "A rule is proven by what it refuses"
description = "A blocking rule says that a gate refuses work that violates it, and that is a claim about the world. What Majordomus accepts as proof of the claim today, a case that is named, in the tree and recorded when it ran, where that proof stops, and the refusal its plan asks for next."
template = "page.html"
[extra]
lang = "en"
related = ["proof-carrying-documentation/", "silence-is-not-a-verdict/", "automation-is-a-capability/", "knowledge-is-derived-a-person-promotes-it/"]
[extra.translations]
cs = "pravidlo-se-dokazuje-odmitnutim/"
+++

A rule of this repository is a versioned document whose front matter carries an id, a statement and a class. The class has two values. `blocking` means a gate refuses work that violates the rule, and `advisory` means a reader is expected to have read it. [ADR 0048](@/adrs/adr-0048.md) (proposed) states the consequence: a blocking class is a claim about the world, and it is either true or it is not. This article follows what the repository accepts as proof of that claim, where the proof stops, and what its plan asks for next.

## A badge for a YAML key

Until 11 September 2026, nothing decided it. Every surface asked one question, whether the rule carried an `x-majordomus` block, and printed a badge from the answer; one line of `product.rs` read `enforced: o.metadata.get("x-majordomus").is_some()`. A rule could name `test/cases/28_no_hardcoded_values.sh` in a section written for a human reader, that case could be renamed, and the rule went on reading as enforced everywhere. The executable's rules module now calls it what it was: a green badge for the presence of a YAML key.

## Three ways to say who enforces it

ADR 0048 lets a rule declare one of three modes, and the shape of its enforcement block is the declaration, so no mode word can be spelled wrong. A dispatched rule names a `validator`, which `lib/doctrine.sh` calls at the commands the rule's `enforced_by` names. A gated rule names `tests` and no validator, and a CI gate or the behavioural suite runs them. A reviewed rule names `reviewed_because`, the reason nothing executable can express it, and a person enforces it.

The reason is the whole declaration, and there is deliberately no flag beside it: a flag can be set, but a reason cannot be set without writing one. [project.clean-room](@/rules/project-clean-room-1.md) is blocking and reviewed, because it is about the provenance of what was written, which no program in the tree can read. An executable proof always wins: a validator makes a rule dispatched, a test makes it gated, and only then is a reason read. A reviewed rule is counted apart on every surface, never as a pass and never as a failure, because that number rising is governance getting weaker.

## What the gate decides

`scripts/ci/rule-proof-check` reads every rule under the rules section, so a rule added tomorrow is measured tomorrow; the repository's object graph indexes {{ <cite_figure name="rule_objects" /> }} rule documents as this page is built. The gate reports two findings, and they fail differently on purpose. `dangling` means the block names a path that is not in the tree. It always fails and is never excused, because a proof that does not exist is worse than none: the rule reads as proven. `unproven` means a rule names neither a validator, nor a test, nor a reason. That debt is held in a baseline that may only shrink, and when this article was written the baseline held no rule.

The class is not an exemption. An advisory rule is asked the same question, because the class says what a violation costs, not whether anyone can tell there was one. The one exemption is narrow: a rule vendored from the standard package and tagged `principle`, which the package pins by content hash and calls a culture sentence with no mechanical expression. It prints how many it exempts on every run, and CI runs it as the `rule-proof` gate of the structure job.

## Ten states, three that never collapse

The gate decides whether a rule names its proof. Whether the proof ran is a second question, and the executable's rules module answers it by joining each rule to the tree and to the ledger of recorded runs that [ADR 0041](@/adrs/adr-0041.md) (proposed) defines: proof is a recorded execution, and inputs unchanged is not proven. [`docs/DOCTRINE.md`](@/docs/doctrine.md) lists the ten states the module reports, from `proven`, a passing run that nothing has changed since, through `stale`, `failing` and `not run`, to `dangling` and `unproven`.

Three of them must never collapse into each other. `proven` is a passing recorded run; `gated` is a mechanism that refuses violations and that a CI gate runs, with no verdict recorded for it; `reviewed` is a person. A rule's state is the weakest of its parts, with one precedence: a failing case makes the rule failing whatever its other cases say. Otherwise one dangling case makes the rule dangling however many of its other cases pass, because the half that does not resolve is the half a reader would have trusted. [ADR 0061](@/adrs/adr-0061.md) (proposed) would record a gate's verdict as an execution, so that `gated` could reach `proven`.

## Where the floor is

The gate's guarantee, [Every rule names what proves it, and every path it names is in the tree](@/guarantees/rule-proof-is-named-and-resolves.md), says what it does not cover: whether the proof is any good. The gate decides that a rule names something and that what it names is in the tree, and a case that asserted `true` would satisfy it.

The gate's own case shows how low that floor is. Case 125 builds a fixture repository, and so that the vendored rules in it do not fail for its sake, it creates an empty file at every case path they name. The gate accepts each of them, because the question it asks is whether the path exists.

The shell floor was a real instance. [project.portable-shell](@/rules/project-portable-shell-1.md) is blocking: bash 3.2 and BSD userland, no associative arrays, no `mapfile`, no GNU-only flags. Until October 2026 it named one case, `test/cases/08_no_forbidden_constructs.sh`, which scans for `eval` and network clients, the constructs `SECURITY.md` forbids, and for none of the ones the rule does. Every check that asked whether the rule names its proof was satisfied by a case that read none of it, while `test/mesh-lab/run` declared two associative arrays. Case 3480 (I3862) now reads every tracked shell unit for associative arrays, `mapfile` and the GNU-only flags it lists, after holding its scanner to a fixture whose answer is known, so a scanner that stopped finding them cannot pass for a clean tree.

## Named is not refused

None of this shows that a violation is refused. The proof accepts any named, present, passing case, so a rule backed only by a case that walks the happy path reads as proven. An audit on 8 October 2026 found the other half: doctor's proof of the wiring was a grep for a dispatch string, and with `|| exit 0` written into `check.sh` doctor still passed. Case 18 is the proof that the wiring verifier works. It breaks each link of the chain in a throwaway copy of the tool and requires doctor to go red, but it shows no validator refusing a violation.

The owner's intent behind the milestone [a-claim-stands-only-on-its-evidence](@/plan/a-claim-stands-only-on-its-evidence.md) sets the bar: a blocking rule is reported proven only by a recorded run of a case in which its violation is refused. [ADR 0188](@/adrs/adr-0188.md), proposed on 18 October 2026 and not accepted by the owner, decides how. A case names the rule whose violation it refuses in a header line of its own, `# majordomus-refutes: <rule-id>`. The older `# majordomus-negative:` header keeps its meaning, a failure mode of a command, gate, script, workflow or capability, and is never read as rule proof. A dispatched validator is shown, in CI, to refuse a declared violation fixture, and doctor keeps its cheap wiring check and says that wiring is all it checks.

Three issues carry the decision. [I3110](@/plan/I3110.md) adds `refusal_shown` to the proof of every blocking rule and ratchets the list of blocking rules with no refusing case. [I3111](@/plan/I3111.md) gives every dispatched blocking rule a minimal violation fixture, runs the real entry point against it in a disposable repository, and fails when a validator accepts it. [I3112](@/plan/I3112.md) makes the validate job count the blocking rules that no recorded run of its own head proves, and makes the rule pages of the UI and of the site stop drawing a rule that is not run, stale or unknown as a pass. When this article was written the plan listed I3110 as ready and the other two as blocked, both depending on it, and no case carried a refutes header, case 3480 included. ADR 0188 expects most blocking rules to start on the list of rules whose refusal is not shown, a list that only shrinks.

## How to check it

`scripts/ci/rule-proof-check` reports both findings and fails on new debt, and `--strict` ignores the baseline. `majordomus-cli rules` reports every rule's state beside what it names, and `bash test/run.sh 125_rule_proof` runs the gate's own case. `majordomus intent explain a-claim-stands-only-on-its-evidence` shows the criterion `blocking-rule-shows-refusal` and its planned proof, and `git grep -l 'majordomus-refutes:' test/cases` lists the cases that declare a refusal.

## What this does not claim

It does not claim that any blocking rule is shown to refuse its violation: under the decision the plan proposes, none is yet. ADRs 0041, 0048, 0061 and 0188 are proposed, and the owner has accepted none of them; the gate and the proof graph they describe are on master and run. How a guarantee is held to a test that names it back is in [Proof-carrying documentation](@/proof-carrying-documentation.md), why a check that never reported is not a pass in [Silence is not a verdict](@/silence-is-not-a-verdict.md), and where the shell floor comes from in [Automation is a capability](@/automation-is-a-capability.md).
