+++
title = "web-sign-in — The deployed surfaces know who is asking, and the identity providers are data"
description = "One canonical identity-provider kind under .ai/repo/identity/providers/ describes each provider as data: its issuer, its endpoints or its discovery document, the scopes requested, the claim that carries a stable subject and the environment variable names its credentials arrive in. A single OpenID-Connect-shaped flow in the executable serves every provider in that directory; Google and Facebook are two objects, not two code paths. Sessions are cookie-borne, signed, short and revocable; identity is attached to a request once, in the router, and every surface — HTTP, Cockpit, OpenAPI, MCP — reads it from the same place. The capability registry's effect classification, not a per-route list, decides what an anonymous reader may do, and the OpenAPI document and the Cockpit both state that decision because both project it from the registry. Striking a provider is deleting one file and regenerating."
weight = 14
template = "milestone.html"
[extra]
plan_id = "web-sign-in"
source = ".ai/repo/project/milestones/web-sign-in.yaml"
+++
