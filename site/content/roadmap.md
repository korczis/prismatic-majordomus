+++
title = "Roadmap"
description = "What comes after v0.1, in the order the milestone graph derives, each version gated by the previous one being accepted rather than by anyone's intention."
template = "roadmap.html"
[extra]
source = ".majordomus/project/milestones/"
next = [["Planned claims", "/guarantees/planned/"], ["Limitations", "/limitations/"], ["Economics", "/docs/economics/"]]
+++
A milestone here is an outcome, not a bucket of tickets. Its status comes from its own issues
*and* from whether the milestones it depends on have been accepted — so finishing every issue
inside a blocked milestone does not release it. That is why the sequence below is derived
rather than written down, and why there is no progress bar on this page.

Each version names the claim it would move out of *planned*. Naming it is not promoting it:
a claim becomes **guaranteed** when an implementation, a behavioural test and a blocking CI
job exist for it, and the guarantees matrix will keep saying *planned* until they do.
