+++
title = "/docs serves this repository's documentation and /swagger serves the Swagger UI, and neither may take the other's mount"
description = "/docs is this repository's own documentation — the Zola site, served by the running executable from the same source that is published to GitHub Pages. /swagger is the Swagger UI. /openapi.json is the document the viewer reads, derived from the capability registry. / is the landing page. Each of those names belongs to exactly one producer, and a change that gives one of them to something else is refused rather than merged."
weight = 139
[extra]
claim_id = "web-namespaces-reserved"
status = "guaranteed"
source = "docs/claims/web-namespaces-reserved.md"
+++
{% raw %}

## What it means

`/docs` is this repository's own documentation — the Zola site, served by the running executable from the same source that is published to GitHub Pages. `/swagger` is the Swagger UI. `/openapi.json` is the document the viewer reads, derived from the capability registry. `/` is the landing page. Each of those names belongs to exactly one producer, and a change that gives one of them to something else is refused rather than merged.

Unbuilt documentation answers `503` naming the command that builds it, not the viewer that used to answer at that path and not a generic failure. A surface that is not built stays in the machine-readable answer with its state, because hiding a route from the people maintaining it hides it from nobody else; the landing page renders it as unavailable rather than offering a link to a certain 404.

## How it works

The mounts are data before they are routes. `web::discover` resolves `/swagger` from `http::swagger::SWAGGER_PATH` and `/docs` from the site configuration's documentation build, and `docs/generated/web.json` carries a `reserved` map from role to path that the generated documentation renders directly — so the published reservation and the enforced one are the same value, not two statements that happen to agree. `web::validate` refuses a duplicate mount within one world and a nested claim the outer surface did not declare, and `http::surfaces` refuses at router construction rather than at request time.

Serving `/docs` is `web::files`, which is not a general file server: a request path is split into segments, each segment is checked before the filesystem is touched, `.`, `..` and empty segments are refused, the resolved path is canonicalised and required to remain inside the canonical root — which closes the door a symlink inside the directory would otherwise open — and only an allowed extension is served. There is no concatenation of untrusted text anywhere in it.

The two mounts are also why the site source states no origin-absolute link. `scripts/site-basepath-check` refuses one, so the same Zola source builds for the Pages origin and for the `/docs` mount without a second copy of the documentation existing anywhere.

## How to see it

```bash
majordomus web list | grep -E 'swagger|docs'    # /swagger and /docs, each with its producer
jq '.reserved' docs/generated/web.json
scripts/site-basepath-check                     # the source is base-path independent
bash test/run.sh 89_web_surface                 # the names, over a real socket
```

## What it does not cover

There is no redirect from `/docs` to `/swagger`. Preserving the old meaning would preserve the collision, which is the thing being removed; a reader who arrives at `/docs` expecting the viewer gets the documentation, which is what the word says.

The reservation is of names, not of content. What the documentation build contains is the site's business.

## Why it exists

The Swagger UI held `/docs` because the word was free when it was mounted and nothing recorded what the word meant. When documentation arrived it wanted the name it had already been given, and nothing in the repository could refuse the collision or even notice it — the mount was a constant in a router, and its meaning lived only in the heads of the people who had read that router. Case `test/cases/89_web_surface.sh` asserts the ownership as data and then over a socket, including that a built documentation mount does not swallow `/swagger` or `/openapi.json` beneath it.
{% endraw %}
