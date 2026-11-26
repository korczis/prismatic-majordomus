# Picture across every project on a machine

Intent: "What is going on across all my projects?" There is no repository in the working
directory, or PROJECT names several. MODE auto resolves to global.

The worker enumerates repositories under the workspace root (any directory that `git
rev-parse` accepts, plus the linked worktrees), and records which are supervised. It runs
stages 2–8 per repository within an equal share of BUDGET, with DEPTH quick unless one is
named, and then merges the results:

- one TL;DR line and one coverage row per repository;
- the links between repositories: shared remotes, a dependency named in a manifest, the same
  tracker project, the same people on the board;
- shared resources that one project's state depends on, such as a CI slot cap or a request
  budget per consumer, as DERIVED findings;
- the cross-project next actions, ranked by what they unblock.

Confidentiality is applied per repository. A finding from one project's external channel
never appears in another project's section.
