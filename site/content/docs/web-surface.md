+++
title = "Web surface"
description = "Every surface this repository exposes over HTTP, and the names that are reserved."
weight = 95
[extra]
source = "docs/generated/web.json"
+++

Everything below is derived from the resolved web topology that the executable
serves from. A surface is declared once, where its producer is; the router, the landing
page, the machine-readable index and this table are projections of that one resolution.
Adding a surface here is not a step — it is what happens.

## Reserved names

These paths mean one thing. `/docs` served the Swagger UI once, and the day real
documentation arrived the name meant two things; the rule
`project.web-surface-declared-once` exists so that cannot recur.

| Role | Path |
|---|---|
| home | `/` |
| documentation | `/docs` |
| swagger | `/swagger` |
| openapi | `/openapi.json` |
| capabilities | `/api/v1/` |

## Surfaces

| Surface | Path | Kind | Category | Offered to | Availability | Producer |
|---|---|---|---|---|---|---|
| The capability registry over HTTP | `/api/v1` | native-route | api | public | served-only | `capability registry` |
| The registry, rendered for a person | `/cockpit` | native-route | interface | public | served-only | `cockpit` |
| The documentation, as this process serves it | `/docs` | static-directory | documentation | public | served-only | `scripts/site-build --serve` |
| MCP over HTTP for attached clients | `/mcp` | native-route | protocol | internal | served-only | `mcp endpoint` |
| The OpenAPI document of the capability registry | `/openapi.json` | native-route | api | public | both | `capability registry` |
| Swagger UI over the OpenAPI document | `/swagger` | native-route | documentation | public | served-only | `http::swagger` |
| The site as it is deployed | `/` | static-directory | documentation | public | published-only | `scripts/site-build` |
| This process, and everything it serves | `/` | native-route | interface | public | served-only | `web::home` |

*Availability* says which world a surface belongs to: `served-only` is answered by a
running process, `published-only` is uploaded as files, `both` is each. *Offered to* is
whether a person is shown it on the landing page; an `internal` surface is still served and
still introspectable, because hiding a route from its maintainers hides it from nobody else.

Ask a running process the same thing at `/api/v1/web/surfaces`, or the repository at
`majordomus web list`; `majordomus web explain` says where each value came from.
