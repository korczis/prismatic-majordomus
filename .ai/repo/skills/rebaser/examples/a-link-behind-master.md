# A link behind master

Train 13c landed and every open pull request went `CONFLICTING` on the forge within minutes.
Bring the next train's members current without spending a check run on any head that does
not need one: refresh the observation, read each member's disposition and relation, merge
master into the `needs_refresh` ones whose conflicts are only derived files with
`prs repair <n> --apply`, leave the heads that already contain master alone, hand the one
`conflicting` member back to its owner naming its authored files, and wait for the stacked
link until its base lands. Report each member with the strategy, the head pushed and the
check run it started.
