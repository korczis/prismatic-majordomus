+++
title = "Roadmap"
description = "What is happening, what can begin and what waits, inferred from the milestone graph and from what each release actually shipped — no version is promised before it exists."
template = "roadmap.html"
[extra]
source = ".ai/repo/project/"
next = [["Planned claims", "/guarantees/planned/"], ["Limitations", "/limitations/"], ["Economics", "/docs/economics/"]]
+++
A milestone here is an outcome, not a bucket of tickets. Its status comes from its own issues
*and* from whether the milestones it depends on are done, so finishing every issue inside a
blocked milestone does not release it. The horizons below follow from those statuses, and the
order inside each one follows from the graph. Nothing on this page was written down as a plan.

No milestone carries a version. A version is a release that happened, so the page shows what
each published release completed. For the future it shows a pace measured over recent releases,
and a forecast only where that pace was measured. Where the records and the released code
disagree, the page says so as a signal. It never changes a status on its own.

A milestone may name the claims it would move out of *planned*. Naming a claim does not promote
it. A claim becomes **guaranteed** when an implementation, a behavioural test and a blocking CI
job exist for it, and the guarantees matrix keeps saying *planned* until they do.
