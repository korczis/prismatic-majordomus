+++
title = "A session does not finish behind its own published site"
description = "A repository that publishes has a second tree — the one the public is being served — and no commit-level gate can measure it, because no change to any file can make it right or wrong. A session that lands work and leaves the publication owed has finished nothing the public can see, and every check stays green while that is true. The gates that measure a deployment are declared as such in the CI model, and finish asks them."
weight = 45
[extra]
id = "majordomus.publication-currency"
source = "share/standard/majordomus/rules/publication-currency.v1.md"
+++
