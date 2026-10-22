# A rule that no program can express says so, with the reason, and is counted apart

## What it means

Some rules genuinely have no machine expression. *The provenance of what was written* is
one: no program in this tree can read where a paragraph of source came from. *Whether a
mechanism earns its cost* is another: that is a judgement, and a check that pretended to
make it would be worse than no check.

Before this guarantee those rules sat in `rule-proof-baseline.txt`, a list of blocking rules
that named no proof — alongside rules that merely had not been got round to. The two were
indistinguishable, and that is the worse failure of the pair: the list could only be read as
work still to do, and half of it never would be.

A rule may now say, in its own front matter, that nothing executable can express it, by
carrying `reviewed_because:` with the reason. That is a declaration a reviewer can audit,
which a line in a debt file is not.

## How it works

The reason is the whole declaration. There is deliberately no flag beside it — a flag can be
set, and a reason cannot be set without writing one — so an exemption is always a sentence
somebody had to defend. A block carrying one word and no reason is not a declaration; it
falls through to `unproven`, which is the finding it was meant to replace.

Two properties keep it from becoming a way out:

**An executable proof always wins.** The mode is decided by what the block names, in order:
a validator makes it dispatched, a test makes it gated, and only then is a reason read. A
rule that acquires a case stops being review-enforced whether or not the reason is still
there, so the declaration cannot outlive its own need.

**It is counted apart, everywhere.** `rule-proof-check` prints every one as `NOTE` on every
run, never as a pass and never as a failure, and states the count beside the measured total.
The typed report counts them in `review_only`, never in `passing` and never in
`named_proof`. This number going up is governance getting weaker, and no summary in this
repository folds it into a total that would hide that.

## How to see it

```
scripts/ci/rule-proof-check     every declared exemption, with the rule that declares it
majordomus rules list           review-enforced: <the reason>, in the last column
```

## What it does not cover

Whether the reason is a *good* reason. Nothing can decide that, and pretending otherwise
would be the fake green this mechanism exists to prevent. What the shape buys is that the
reason exists, is attached to the rule it excuses, and is read by whoever reviews the
change — the exemption is visible and attributable rather than a line in a list.

It also does not stop the number growing. A repository could declare every rule
review-enforced and this check would pass. What it guarantees is that doing so is loud:
the count is printed on every run and counted apart from every rule that has a gate, so the
growth is a number someone has to look at rather than a silence.

## Why it exists

Because the previous mechanism could not tell two different things apart. `rule-proof-baseline.txt`
called itself "blocking rules that name no executable proof: enforced by review today", and
for four of its ten entries there was no day on which that would stop being true. A list
that mixes *nobody can automate this* with *nobody got round to it* makes the second
invisible and the first look like negligence.

## What proves it

`test/cases/125_rule_proof.sh`: a fixture rule declares a reason and is reported as
`reviewed` and not as a failure; the same rule with the reason removed is refused by the
loader; a rule declaring both a validator and a reason is refused as a contradiction; and a
rule that gains a case is reported as gated with the reason still in place.
