+++
title = "Roadmap"
description = "What comes next, in the order the milestone graph derives, each milestone gated by the ones it depends on being accepted rather than by anyone's intention."
template = "roadmap.html"
[extra]
source = ".majordomus/project/milestones/"
next = [["Planned claims", "/guarantees/planned/"], ["Limitations", "/limitations/"], ["Economics", "/docs/economics/"]]
+++
A milestone here is an outcome, not a bucket of tickets. Its status comes from its own issues
*and* from whether the milestones it depends on have been accepted — so finishing every issue
inside a blocked milestone does not release it. That is why the sequence below is derived
rather than written down, and why there is no progress bar on this page.

A milestone that has shipped shows the release that carried it: the first release tag
containing its last accepted issue, inferred from the tags and never typed. Each milestone
names the claim it would move out of *planned*. Naming it is not promoting it:
a claim becomes **guaranteed** when an implementation, a behavioural test and a blocking CI
job exist for it, and the guarantees matrix will keep saying *planned* until they do.
