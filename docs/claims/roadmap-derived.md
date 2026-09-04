# The roadmap is derived from milestone state, and no document may be a second authority for it

## What it means

There is no roadmap file. The roadmap is the milestone graph read in dependency order, and
`majordomus plan roadmap` computes it on every call. A milestone's position comes from what
it requires, not from where someone typed it in a list.

While a hand-written roadmap table still exists anywhere — as one does in `README.md` until
the website's roadmap route replaces it — that table is held to the model in both
directions. It cannot list a version no milestone declares, and it cannot hide one the model
does. It is allowed to be a summary; it is not allowed to be an authority.

## How it works

`mj_validate_roadmap` reads the versions the model declares from the loaded milestone
records, and the versions the `## Roadmap` section of `README.md` lists. A version in the
document that no milestone declares is a document inventing a release. A version in the
model that the document omits is a document hiding one. Either is a blocking failure under
`doctor` and drift under `watch`.

When the section is gone the check passes without comparing anything. That is deliberate:
the doctrine refuses the regression rather than requiring the table.

## How to see it

```
majordomus plan roadmap          # the sequence, computed from the graph
majordomus doctor                # OK roadmap README.md — every version it lists is a milestone
```

Add a row for a version no milestone declares and `doctor` exits 10 naming it:

```
FAIL roadmap     README.md — lists version(s) no milestone declares: 2.0
```

## What it does not cover

It compares version strings, not prose. A roadmap table whose descriptions have gone stale
against the milestones they describe will still pass, because there is no way to check a
sentence against an outcome without reading both. The durable fix is not a better check but
the absence of the table: the website's roadmap renders the milestone records themselves.

It also says nothing about documents it does not know about. A roadmap pasted into a wiki or
a slide is beyond anything this repository can see.

## Why it exists

The roadmap was a Markdown table scraped into the website by the site generator, and the
sentence "each step is gated by the previous one being real" sat under it with nothing
enforcing the gate. Two authorities for the same fact, one of which was prose.

The milestone model replaced the table as the source. This doctrine is what stops the table
coming back — because the failure mode is not someone deciding to abandon the architecture,
it is someone adding a row to a table in a hurry and nothing objecting.
