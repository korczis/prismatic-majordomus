# The documented one-line install command is composed from the model's parts, and a document that states a different one fails the suite

## What it means

The command a person pastes is not a string anybody typed into a document. It is four
fields of `share/distribution.yaml` — the download command, the base URL, the script's name
and the shell — composed in one place:

```text
{download_command} {base_url}/{script} | {shell}
```

`README.md`, `docs/INSTALL.md`, the landing page, the getting-started page and the
installer's own help text all show that composition. The website and the guide are
generated from it; the README is checked against it.

## How it works

`Model::install_command` composes it, `majordomus distribution show` answers it, and
`site/data/registry/distribution.json` carries it for the templates. The behavioural case
`test/cases/84_distribution_model.sh` asks the model for the command and requires it to
appear verbatim in `README.md` and in the installation guide.

## How to see it

```bash
majordomus distribution show --format json | grep install_command
bash test/run.sh 84_distribution_model
```

## What it does not cover

The prose around the command is written by people and is not generated. What is guaranteed
is that the command itself cannot quietly differ between the README, the guide and the
website — including after the base URL or the script's name changes.

## Why it exists

A typo in an installation command is a uniquely expensive documentation bug: it fails for
every reader, it fails at the first thing they try, and it is invisible to whoever wrote it
because they never paste it. Deriving it makes the typo impossible; checking the one
document that is not derived makes it fail the suite instead of the reader.
