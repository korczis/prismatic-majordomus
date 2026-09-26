# Redaction fixture

`shapes.tsv` is the one set of samples both redactors are held to: the shell's
`mj_capture_redact` in `lib/capture.sh`, which the prompt archive uses, and its Rust port,
`majordomus_cli::redaction::redact_secrets`, which published evidence goes through. Test case
`504_public_text_is_redacted_once` runs every line through the shell, and
`apps/majordomus-cli/tests/redaction.rs` runs the same lines through the port. Each checks
the expectation the line states, so the two agree on every line without either having to
call the other.

## Columns

| column | meaning |
|---|---|
| `name` | the shape the line exercises: a name from `MJ_CAPTURE_SECRETS`, or `assignment` |
| `prefix` | the start of the sample: a credential's own prefix, or the name and separator of an assignment |
| `body` | the rest of the sample |
| `expect` | `redacted` or `kept` |

A line is assembled as `prefix` followed by `body` and redacted whole. A `redacted` line
must come out as `[redacted:<name>]`, except an `assignment` line, which keeps its name and
separator: it comes out as its `prefix` followed by `[redacted:assignment]`. A `kept` line
must come out unchanged. Both checks also require every shape to have at least one line of
each kind, so a fixture that loses a shape fails rather than passes.

## Why the samples are split

No committed file may carry a string in a credential's shape. GitHub push protection scans
what is pushed, and this repository's own gates refuse a credential marker in anything they
publish; a fixture that held whole samples would trip the first and invite an exemption from
the second. Split by a tab, a prefix and a body are harmless apart, since no shape crosses a
tab, and they become a sample only in memory, at run time. The bodies are runs of one letter,
so that nothing here can be mistaken for a real credential even once assembled.

## What the lines cover

- Every shape at its minimum length, and longer where the shape allows it. The AWS and
  Google key shapes have an exact length rather than a minimum, so a longer body is not a
  whole-line match; they are held at their length, and the port's unit tests hold what
  happens to the rest of a longer run.
- The assignment rule in several casings and with the separators people write: `=`, `:`,
  spaces around either, and a quote on either side.
- Near-misses that must be kept: a body one character short of its shape, a prefix that is
  almost right, and prose that merely mentions a password.

Machine paths for the normalisation tests are not here. They are built at run time from a
temporary directory, because a committed path naming a home directory is exactly what
`scripts/ci/no-machine-paths` refuses.
