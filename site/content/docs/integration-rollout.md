+++
title = "Integration rollout record"
description = "the record of each integration rollout stage of ADR 0101 §13: what was run against the real repository, with which released executable and under which rule and ADR versions, what it answered, and that nothing moved"
weight = 54
[extra]
source = "docs/INTEGRATION_ROLLOUT.md"
+++

{% raw %}

The record of each stage of the integration rollout that ADR 0101 §13 stages and
[`INTEGRATION.md`](@/docs/integration.md#rollout) lists. Each stage is taken against the real
repository with a released, installed executable and records what was run, what it answered
and what moved. A dry run writes nothing to the audit trail, so for stage 1 this file is the
record. Stages 2 and 3 are recorded on the trail itself, as merges the continuous-mode gate
counts.

## Stage 1: dry run, 2026-10-06

**Taken under:** rule `project.integration-follows-the-current-master` version 2, rule
`project.land-and-publish` version 1, and ADR 0101 (status `proposed`, dated 2026-09-30, as at
master `5908234ce0`).

**Executable:** majordomus 0.14.0, the published release, installed by the advertised installer
into a prefix of its own, so no other installation was used or changed:

```bash
sh install.sh --version 0.14.0 --prefix <prefix> --install-dir <prefix>/bin
<prefix>/bin/majordomus --version        # majordomus 0.14.0
```

**Repository:** `korczis/prismatic-majordomus`, from a clone of it (`<repo>`) with
`MAJORDOMUS_BIN` and `MAJORDOMUS_SHARE` unset, so the installed release answered and not a build
of the tree. The forge's master was `5908234ce001f521e22c2534f837ee939b70a7c6`, the commit that
records the 0.14.0 release.

<!-- majordomus:unrun this is the transcript of one run against the real forge, which needs the network and a token; the integration tests run these commands against a scripted one -->
```bash
cd <repo>
majordomus prs refresh            # exit 0
majordomus prs status             # exit 0
majordomus prs drain --dry-run    # exit 0
majordomus prs prove-dry-run      # exit 0
```

### What it answered

`prs refresh`:

```text
observed 16 open pull request(s) of korczis/prismatic-majordomus at 2026-10-06T15:10:10Z; master is 5908234ce0
```

`prs status`:

```text
korczis/prismatic-majordomus · master at 5908234ce0 · observed 2026-10-06T15:10:10Z (16 open)
lanes: held 2 · repair 6 · waiting 8
rank  pr     disposition            risk   reason                             title
   1  #746   needs_refresh          medium behind_master:171                  fix(finish): a note is read once, and kept before the outco…
   2  #789   needs_refresh          medium behind_master:134                  fix(generate): the registry counts the web surfaces every c…
   3  #747   needs_refresh          medium behind_master:197,required_checks… fix(server): a server serving code that no longer exists st…
   4  #743   needs_refresh          medium behind_master:232,required_checks… feat(cockpit): the plan, one milestone and one issue, with …
   5  #742   needs_refresh          high   behind_master:233,required_checks… fix(lint): a reporter that sets a flag is a reporter
   6  #741   needs_refresh          high   behind_master:233,required_checks… feat(pack): a source pack for ChatGPT, verified to carry on…
   7  #739   needs_refresh          high   behind_master:246,required_checks… feat(continuity): a handover moves between machines as a si…
   8  #667   waiting_for_dependency high   stacked_on:#650,conflicts_on:3,re… feat(evidence): a recording carries what its run measured, …
   9  #745   needs_repair           medium required_check_failed,behind_mast… feat(timing): the timing report is one line of JSON under -…
  10  #732   needs_repair           medium required_check_failed,behind_mast… feat(site): one status vocabulary, the challenge as proof, …
  11  #734   conflicting            high   conflicts_on:1,required_check_fai… feat(site): one feature card; /features/ is the product by …
  12  #785   conflicting            high   conflicts_on:1,required_check_fai… fix(release): prose names no version label, and the limitat…
  13  #788   conflicting            high   conflicts_on:3,required_check_fai… integrate(batch-n3): features by domain, one status vocabul…
  14  #292   conflicting            high   conflicts_on:20,required_checks:m… feat(completion): done is one policy, a derived stage, and …
  15  #650   other_base             medium base_is:feature/evidence-is-judge… feat(evidence): a crate binary that ran nothing is not a pa…
  16  #552   other_base             medium base_is:fix/the-cockpit-areas-are… feat(cockpit): the plan, its milestones and issues, the ses…
```

`prs drain --dry-run`:

```text
idle: nothing is ready; 7 pull request(s) need master brought in first: #746 #789 #747 #743 #742 #741 #739
stopped: nothing is ready; 7 pull request(s) need master brought in first: #746 #789 #747 #743 #742 #741 #739
```

`prs prove-dry-run`:

```text
refresh          observed 16 open pull request(s); master is 5908234ce001f521e22c2534f837ee939b70a7c6
plan             nothing is ready to merge
drain --dry-run  idle: nothing is ready; 7 pull request(s) need master brought in first: #746 #789 #747 #743 #742 #741 #739; nothing is ready; 7 pull request(s) need master brought in first: #746 #789 #747 #743 #742 #741 #739
cleanup          nothing to close
observed classification (base master):
  #746  0594c009784a  needs_refresh  bring master in with the derived merge driver, derive, push; CI runs again
  #789  e6392cc17476  needs_refresh  bring master in with the derived merge driver, derive, push; CI runs again
  #747  8c29f1f3a63a  needs_refresh  bring master in with the derived merge driver, derive, push; CI runs again
  #743  a23e632a31fc  needs_refresh  bring master in with the derived merge driver, derive, push; CI runs again
  #742  15f95faf92d3  needs_refresh  bring master in with the derived merge driver, derive, push; CI runs again
  #741  748b4cd22930  needs_refresh  bring master in with the derived merge driver, derive, push; CI runs again
  #739  fd36929ae0df  needs_refresh  bring master in with the derived merge driver, derive, push; CI runs again
  #667  e3582b1d8fcf  waiting_for_dependency  land #650; its merge retargets this onto master
  #745  3142d2762746  needs_repair  the author fixes the failing required check
  #732  c3a04f44134f  needs_repair  the author fixes the failing required check
  #734  8f7ec3cc89c1  conflicting  the author merges master and resolves site/templates/feature.html
  #785  f76abc08f699  conflicting  the author merges master and resolves docs/README.md
  #788  4153efdd4a5a  conflicting  the author merges master and resolves apps/majordomus-cli/Cargo.lock, apps/majordomus-cli/Cargo.toml, site/templates/feature.html
  #292  1d22b89dd7dc  conflicting  the author merges master and resolves .ai/repo/evidence/ledger.json, README.md, apps/majordomus-cli/src/capability/builtin/gates.rs, apps/majordomus-cli/src/cockpit/mod.rs, apps/majordomus-cli/src/cockpit/nav.rs, apps/majordomus-cli/src/gates/done.rs, apps/majordomus-cli/src/gates/mod.rs, apps/majordomus-cli/src/release/version.rs, apps/majordomus-cli/tests/cockpit.rs, docs/DISTRIBUTION.md, docs/README.md, docs/SCHEMAS.md, lib/derive.sh, lib/finish.sh, scripts/generate-site-data, scripts/lib/executable-site.jq, site/data/homepage.toml, site/data/marketing.toml, site/data/nav.toml, site/templates/index.html
  #650  de2945d9a660  other_base
  #552  85188af5d085  other_base
nothing moved (remote, forge, trail, lease, local refs)
```

### What it shows

- **Nothing moved.** The proof checks the remote, the forge, the trail, the lease and the local
  refs before and after, and finds every one unchanged. The observation carried no diagnostic.
- **Nothing was ready, so stage 2 has nothing to merge yet.** Of the 16 open pull requests,
  seven are `needs_refresh`: master is what they lack, and their only conflicts with it are
  derived files. One waits for the pull request it is stacked on, two need their failing
  required check repaired, four conflict on authored files, and two target another base.
- **The dispositions were checked by hand.** For example, #746 is `needs_refresh` and not
  redundant: `git cherry` finds none of its three commits on master, and master's
  `lib/finish.sh` still copies the note after the outcome is written. #788 conflicts on
  `Cargo.toml` and `Cargo.lock`, which the 0.14.0 release changed on master after the branch
  last took master in.
- **Stage 2 needs one `ready` pull request.** `prs repair <n> --apply` brings master into one
  of the `needs_refresh` pull requests (or `prs drain --refresh` brings in the first), and stage
  2 can be taken once its required checks pass on the new head.
{% endraw %}
