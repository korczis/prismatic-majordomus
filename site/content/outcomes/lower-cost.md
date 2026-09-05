+++
title = "Lower cost"
description = "Make AI development cost less and move faster."
weight = 2
[extra]
moments = ["strongest-model-renames-a-variable", "two-agents-one-bug", "done-because-the-model-said-so"]
commands = ["start", "check", "finish", "doctor"]
claims = ["capability-class", "profile-axes", "overlap-report", "scope-enforcement", "finish-contract"]
+++
{% raw %}
## The situation

AI spend rarely goes on the hard part of the work. It goes on the strongest model running
at maximum effort to rename a variable, on sessions that re-read the whole repository before
touching anything, on two agents fixing the same bug in two branches, and on work that was
called done, merged, and had to be done again the next morning.

None of that is a model problem. It is what happens when nobody decides how much effort a
task deserves, what a worker may touch, and what "done" means before the work begins.

## What Majordomus changes

Each class of task runs under a named profile that sets the capability class, the reasoning
effort, the verbosity and the context a worker gets, independently. A quick fix and an
architectural change no longer cost the same. A task declares the paths it may touch before
the first edit, so two workers heading for the same files are reported instead of
discovered at merge time. And "done" is a contract evaluated line by line: work that does not
meet it is refused and stays open, rather than being accepted and paid for twice.

Durable state replaces transcripts, so the next session does not buy the context back from
the model at full price.

## Where the money goes

What is paid for is model effort and people's hours, and a large share of both is spent on
rebuilding context, on duplicated work, and on rework after a premature "done". Majordomus
removes those repeated costs. It does not change the price of a token, and it does not
claim a saving it has not measured: the ledger already records sessions, handovers and
verification per task, and cost per accepted outcome becomes a query once token telemetry
is recorded.

## What this does not promise

Majordomus never calls a model and never chooses one for you; a profile names a capability
class, and your tooling maps it to a vendor. It does not stop a worker from editing a file
while it runs; it refuses to accept the result. The pages linked below say which commands do
this and what is guaranteed, advisory, or still planned.
{% endraw %}
