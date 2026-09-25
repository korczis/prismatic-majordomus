# The crate's rustdoc is a web surface at /rustdoc, declared from the crate and served from the tree its producer wrote

## What it means

The reference rustdoc renders from the crate is one of the surfaces this repository serves, with
the id `rustdoc` and the mount `/rustdoc`. It is in the topology from the moment the crate is:
`majordomus web list` names it, the home page of a running server offers it, and the committed
projection `docs/generated/web.json` carries it, whether or not anybody has produced the pages
in this checkout. A running `majordomus serve` answers `/rustdoc/` from the tree the producer
wrote, and when there is no tree yet it answers `503` naming the command that writes one
instead of an unexplained 404.

## How it works

`web::discover` declares the surface from the crate, the way it declares the served
documentation from `site/config.toml`: the declaration depends on the crate existing, not on a
build having happened. The walk over the generated web root skips the `rustdoc` directory for
the same reason it skips `docs` — discovery already knows the surface — and reads only the
revision the producer recorded there. That revision, `built_from`, is left out of the committed
projection, because a value that differs between checkouts would make every clone report the
projection as drifted.

The producer, `scripts/rust-check --doc`, writes `target/web/rustdoc/` with a `surface.json` in
`web-surface/v1` and a landing page. Serving goes through `web::files`, the guarded static server
that already serves `/docs/`: every segment of the request path is checked before the filesystem
is touched, and the resolved file must stay inside the canonical root.

## How to see it

```bash
majordomus web explain rustdoc                          # mount, producer, availability, built_from
jq '.surfaces[] | select(.id == "rustdoc")' docs/generated/web.json
scripts/rust-check --doc && majordomus serve            # then /rustdoc/ on the URL it logs
bash test/run.sh 488_rustdoc_discoverable
cargo test --manifest-path apps/majordomus-cli/Cargo.toml web::discover
cargo test --manifest-path apps/majordomus-cli/Cargo.toml --test http_serve
```

## What it does not cover

Whether the tree it serves is complete or current is `rustdoc-complete` and
`rustdoc-verified-live`; this claim is about the surface existing and being reachable. What the
published site carries is `rustdoc-composed-from-the-topology` and `rustdoc-published`. The
mount itself — why `/rustdoc` and not a path under `/docs` or `/api` — is a decision, recorded in
ADR 0086, not something a case can prove right.

## Why it exists

For as long as the crate existed, its gate rendered this reference on every run and nothing
declared it: the router could not serve it, the home page could not link it, and the topology the
site is built from did not name it. A surface that exists only when a producer happens to have
run in one checkout is a surface nobody can depend on, so the declaration is taken from the crate
itself. `test/cases/488_rustdoc_discoverable.sh` asks the topology before the producer has run
and a served socket after it.
