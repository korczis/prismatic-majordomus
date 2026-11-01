+++
title = "deploy.slug"
description = "Writes, into a directory outside the checkout that is absent or empty, the tree the deployment's provider receives (ADR 0219): the object's build inputs except the package the executable is compiled from, read from git at one commit (HEAD unless named); the provider's generated files of that commit at the slug's root, each held to what the object renders and the hooks made executable; the provider's buildpack line; the executable built at that commit; and a record naming the deployment, the commit, the executable's digest and the provider. Refuses an out directory inside the checkout or not empty, a deployment that names no provider, a build input the commit does not track, a provider file the object does not render, and two files that would land on one path. Changes nothing in the repository and reaches no network; pushing the slug is the provider workflow's."
weight = 32
slug = "deploy-slug"
[extra]
id = "deploy.slug"
source = "apps/majordomus-cli/src/capability/builtin/deploy.rs"
+++
