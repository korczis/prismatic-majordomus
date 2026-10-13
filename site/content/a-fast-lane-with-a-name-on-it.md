+++
title = "A fast lane with a name on it"
description = "The owner of this repository ordered its sessions to merge past the required check. What twenty-one hours of doing that broke, the ten clauses that came out of it, and the one thing the order never reaches."
template = "page.html"
[extra]
lang = "en"
+++
{% raw %}

On 12 October 2026 the published site of this project named a release two versions behind the code for most of a day. Nothing in the work was broken. The owner wrote: "almost 24h with no release on gh pages + binaries, this is tragic".

The cause was arithmetic. The required check takes about an hour on the hosted runners. The pool judges one pull request at a time. A record pull request that changed one file waited two and a half hours behind runs that nobody was going to read. Several sessions were producing work faster than one queue could judge it.

The owner then gave an order, on 12 October and again on 13 October, in these words: "deploy using --admin, bump, deploy, verify, rinse and repeat" and "use --admin mode, merge and deploy asap, review and cancel staled jobs, be agressively efficient in delivery".

This repository already had a rule that said a pull request is never merged with `--admin`. Both statements are still true, and this article is about how.

## What the order broke

The sessions followed the order for twenty-one hours. Each failure below happened and was measured.

A branch whose own checks were green turned the trunk red seven times in one evening, because the trunk had moved under it. The pair that failed existed only in the merged tree: two cases with one number, a gate with no job to run it, a documentation link to an item another branch had made private.

Several sessions were landing at once. A merge that arrives between another lander's gates and its merge turns those gates into a verdict on a tree that no longer exists.

One landing went in with the structure job unfinished. Its two reds reached the trunk, and nothing had said the job was unfinished.

A branch's run kept going after its pull request merged. The trunk's run of the merged commit waited behind it.

Twice in one night two sessions built the same fix for one red. The second build took the machine to one gibibyte of free disk.

The record of a release waited two and a half hours behind landings that moved the derived files under it. The site named the previous release for all of that time.

## Ten clauses

The rule is `project.an-expedited-landing-is-the-owners-word`. Each clause answers one of those failures.

1. The authorisation is the owner's and is recorded, with its date. A session does not infer it, and does not take it from another session's relay.
2. One named lander at a time holds an exclusive claim for the landing and releases it after.
3. The head contains the current trunk and the merge is pinned to it:

   ```sh
   gh pr merge --admin --merge --match-head-commit <head>
   ```

   A head that moved is refused by the forge.
4. The merged tree is judged before the merge, and the judgement is named: which gates ran, on which commit. A gate that did not run is named as not run.
5. The integration executor never passes `--admin`. The old rule stays true for every line of code here. The exception is a person's order carried out by a session, never a default.
6. Every landing is verified where the public reads it.
7. A run nobody will read is cancelled. Four kinds never are: the trunk's only run of a commit, a deployment in flight, a release run, and another session's run on a head that is still current.
8. A red on the trunk has one carrier.
9. Between a tag and its record nothing else lands.
10. A release is never expedited. A tag is pushed only on a commit whose required check concluded success.

The last clause is the reason the rest is acceptable. A merge is corrected by the next merge. A tag publishes binaries that people install.

## What a test can hold, and what it cannot

`test/cases/2999_delivery_is_expedited_only_where_the_rule_says.sh` reads the workflows and the rule themselves. It fails when the release workflow publishes without asking for the verdict of the required check, when a landing on the trunk could cancel the trunk's run or a deployment in flight, when the integration code names `--admin`, or when the documented merge command loses its pin.

It was proved able to fail twice. With the verdict step removed from the release workflow it failed and named each publishing step that was no longer behind the verdict. With `cancel-in-progress: true` in the validation workflow it failed and named the file.

Five clauses are acts of a session, and no case holds them: who the owner authorised, who holds the claim, which gates the lander names, who carries a red, and what lands between a tag and its record. They are decided by the record: the claim on the mesh, the gates named in the pull request, the board. Cancelling stale runs is still done by hand. A capability that lists them and cancels only what the rule allows is the next step, and until it exists clause 7 depends on a session reading the run list.

## The first day under it

On 13 October, v0.23.0 was tagged at 12:15 UTC, on a commit whose required check had succeeded. Its record merged at 13:10 UTC. No session landed anything in those fifty-five minutes, and the record needed no repair. The installer, run in an empty home directory, answered `majordomus 0.23.0`.

The rule's own pull request was landed the way the rule says. The trunk was merged into the branch and the merge committed. All 53 gates of the structure job ran on that head. One failed on the first run, for a reason that was the lander's own stale checkpoint and not the tree, and passed when rerun on the same head. The pull request names one case as not judged, with the reason. The merge was pinned, the claim was released, and the branch's own run was cancelled.

The decision is ADR 0155. Its status is `proposed`, because accepting a decision is the owner's act and not a session's. The manual is [`docs/INTEGRATION.md`](@/docs/integration.md), section "An expedited landing".
{% endraw %}
