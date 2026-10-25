+++
title = "Coverage and CI cost over time"
description = "The coverage each master run measured and what each run cost in minutes, read from the runs themselves at every publication."
template = "trends.html"
+++
Coverage and CI cost are measured on every run of master's validation. This page keeps neither:
it reads them again from the runs themselves each time the site is published, so a slow drift
is visible here rather than only to a person reading runs one at a time.

Each point is one completed run of master. The coverage of a run is the crate's totals as
`scripts/rust-coverage` measured them in that run, with test code excluded, taken from the
run's own `coverage-summary` artifact; nothing here counts coverage a second time. The cost of
a run is what the forge recorded for its jobs: how long each waited for a runner and how long
it ran. A run whose artifact the forge no longer keeps still shows its cost, and its coverage
reads as not recorded, never as zero.
