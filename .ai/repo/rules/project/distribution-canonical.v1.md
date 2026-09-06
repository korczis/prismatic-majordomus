---
id: project.distribution-canonical
version: 1
kind: rule
title: Distribution has one canonical model
description: Supported targets, artifact names, installer resolution, the release build matrix, the documentation, the website and the exposed API metadata are derived from share/distribution.yaml or mechanically validated against it; none of them states a platform, a name or an installation URL of its own.
statement: Distribution has one canonical machine-readable model; every surface that names a platform, an artifact or an installation URL derives from it or is checked against it, and adding or removing a supported target propagates to the release build, the installer, the tests and the documentation without any of them being edited.
status: active
class: blocking
depends_on: [project.interfaces-are-projections@1, project.derived-files-regenerated@1, project.no-claim-without-test@1]
tags: [distribution, release, architecture]
---

# Rationale

A distribution model is the textbook shape of the failure this repository exists to catch.
The same fact — that a platform is supported — has to be true in the release build matrix,
in the installer's target table, in the documentation's platform table, on the website, and
in whatever the tool says about itself. Written down five times it is wrong within a
release: the tag builds five artifacts and the installer offers six, or the documentation
promises a platform nothing built, and the failure is discovered by a person who cannot
install the tool.

The cost of the copies is not tidiness either. It is that adding a platform stops being one
edit and becomes a checklist, and a checklist is a thing a hurried person completes
partially.

# Required behaviour

`share/distribution.yaml` is the one model: the binary, the repository releases come from,
the installer's canonical URL and defaults, the archive naming function, and every target
with its operating system, architecture, C library, Rust target triple, status and the
runner a release builds it on. A target it does not declare does not exist, for anything.

Every other surface derives from it or is checked against it:

- the release build matrix is `docs/generated/distribution-matrix.json`, generated from the
  model; the workflow reads it and lists no runner and no platform;
- the installer's platform table is a generated region inside `share/install/install.sh.in`;
  the behaviour around it is hand-written and states no platform;
- the installation guide's platform table is generated into `docs/INSTALL.md` from
  `share/install/INSTALL.md.in`;
- artifact names come from one function, `Target::artifact_name`, reached by every caller
  through `majordomus distribution artifact`; no script, workflow or document composes one;
- the website renders `site/data/registry/distribution.json`, generated from the model;
- the public release metadata is generated from the release records under
  `.ai/repo/releases/`, and the stable pointer is derived from them on every generation and
  authored nowhere;
- the documented install command is composed from the model's parts and appears verbatim in
  `README.md` and `docs/INSTALL.md`, which is checked rather than trusted.

Two further invariants follow from the same model. A release publishes an artifact for
every target the model marks supported, or it is not published: a partial release is
refused when it is recorded, not discovered when someone cannot install. And a public
installation instruction may name only a path that is tested: the installer's own cases
install a real archive from real metadata, and the release pipeline installs the release it
just published from the URL the documentation prints.

# Failure behaviour

`majordomus distribution validate` exits 10 naming every violation of the model's own
invariants and every disagreement between a release record and the model.
`majordomus generate --target distribution --check` exits 10 naming every projection that
has fallen behind, and is reached by `just derive-check`, by the CI gate the `site` job
runs and by the release pipeline before anything is built. `scripts/release-record` refuses
to write a record that is missing a supported target. `scripts/release-version --check`
refuses a tree in which the crate, the shell tool and the tag do not state one version.

# Verification

`test/cases/84_distribution_model.sh` proves the invariants by mutation: a target added to
a copy of the model must appear in the installer, the matrix and the guide, and must leave
the committed projections failing `--check`; a duplicate id, a duplicate triple and a
non-HTTPS base URL must each be refused by name. `test/cases/85_installer.sh` installs a
real archive from real metadata over a real connection and holds every failure path to
leaving the previous installation working. `test/cases/86_installer_platform.sh` resolves
every declared platform from one machine and holds the refusals to naming what they
detected. `test/cases/87_release_pipeline.sh` holds the workflow to the model it adapts.
The design is `docs/DISTRIBUTION.md`.
