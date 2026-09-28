+++
title = "Token economics"
description = "What a coding session consumes with Majordomus and without it, from matched runs judged by the same hidden tests. Every number is generated from recorded evidence and labelled by how it was obtained."
template = "economics.html"
+++
Majordomus does not publish a token-saving figure it has not measured. Everything below is
rendered from `docs/generated/economics.json`, which one calculator derives from the recorded
benchmark runs; nothing on this page is typed by hand, and `majordomus economics check`
refuses a savings number that is.

Two measurements answer different questions. The **total-token reduction** comes from matched
live sessions — the same task, model and hidden acceptance tests, once without Majordomus and
once with it — and is the only kind of number that could support a claim about what
Majordomus saves. **Context selection** counts what the context compiler selects out of what it
judged relevant; no model is involved, and it is not a saving.

Every token figure below is a reduction, `1 - treatment / control`: positive means Majordomus
used fewer tokens, and a negative reduction means Majordomus used more tokens. It is shown as
it is, with the sign the calculator gave it.

The methodology, the control and treatment definitions and the reproduction commands are in
the [economics document](/docs/economics/).
