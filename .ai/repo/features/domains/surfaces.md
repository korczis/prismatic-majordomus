---
schema: domain/v1
id: surfaces
kind: domain
title: Surfaces
headline: 'One declaration in the repository becomes the command line, the HTTP API, MCP, the Cockpit, the documentation and this site, and a projection that disagrees with it fails the build.'
problem: 'Interfaces disagree. The command line, the API, the docs and the website each keep a copy, and the copies drift.'
status: stable
weight: 60
tags: [surfaces, interfaces, projection]
---

# Surfaces

How the repository's declarations reach the people and tools that read them: a capability
declared once and projected to every interface, the Cockpit that renders the registry for a
person, the installer, and the catalogue of failure modes this product answers. None of the
interfaces is maintained by hand, and a projection that has drifted from its declaration is
a build failure rather than a support ticket.

Not in this domain: the rules those interfaces carry, which is governance; and what is true
about them, which is evidence.
