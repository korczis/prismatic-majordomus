+++
title = "Deployment"
description = "The deployments this repository declares, read from the canonical objects the index holds, and whether they would work — decided against the capability registry this process built and the workspace it sits in. Every operation is a read: a deployment is changed by the trusted command line and by CI, never over HTTP and never by an MCP client."
weight = 4
slug = "deploy"
[extra]
id = "deploy"
source = "apps/majordomus-cli/src/capability/builtin/deploy.rs"
+++
