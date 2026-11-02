+++
title = "HTTP API: executions"
description = "Running a capability of this registry as work that can be watched: started, followed event by event over the live channel, read back afterwards, and asked to stop. One plane per checkout: `majordomus run` executes in its own process and reports the execution to the server already serving the checkout, so that server lists it beside its own; with no server it says that nothing else can see the run. In memory; an execution does not outlive the server that holds it."
weight = 1023
path = "docs/api/executions"
template = "api-tag.html"
[extra]
api_tag = "executions"
api_slug = "executions"
source = "docs/generated/openapi.json"
+++
