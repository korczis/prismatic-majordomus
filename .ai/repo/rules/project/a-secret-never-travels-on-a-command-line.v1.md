---
id: project.a-secret-never-travels-on-a-command-line
version: 1
kind: rule
title: A secret never travels on a command line
description: A value read from a credential source, or named as one, is handed to a child process on its standard input, in its environment or through a file of mode 0600 it reads, and never as an argument, which every user of the host can read for as long as the child runs.
statement: Give a child its secret on standard input, in its environment or in a 0600 file, never in its argv; a line that hands over a value which only looks like one says why with `secret-argv: <why>`.
status: active
class: blocking
depends_on: [project.executions-carry-no-secret@1]
tags: [security, rust, shell]

x-majordomus:
  tests: [test/cases/3996_a_secret_never_travels_on_a_command_line.sh, apps/majordomus-cli/src/quality/secret_argv.rs, apps/majordomus-cli/src/mesh/cache_provision.rs]
---

# Rationale

The arguments of a process are public on the host it runs on. Any user reads them with `ps`
or from `/proc/<pid>/cmdline` while the process lives, and audit logs, crash reports and
shell histories keep them after it ends. A file mode cannot protect them. A secret is safe
only where the operating system limits who reads it. A process's environment is readable by
its own user and by root. A pipe is readable by the two ends of it. A file of mode 0600 is
readable by its owner.

The defect that led to this rule was a careful one. I2674 generated each node's MinIO
secret on the host, kept it in a mode 0600 file and never printed or logged it. It then
passed the secret as the last argument of `mc admin user add`. All the care stopped at the
process boundary, and no review saw it, because nothing checked for it.

# Required behaviour

A command line is never built from a secret. In this repository's Rust, that covers
`.arg(..)`, `.args(..)`, `Command::new(..)`, a vector named `argv`, `args` or `cmd`, and the
argument list given to a runner. In its shell, it covers any argument of an external
command. A secret is a value read from a credential source: a call or a variable named as a
secret, an `env::var` of one, a keychain or credentials-file read, or a binding filled from
one.

The forms that pass are:

- the child's standard input (`mc admin user add <alias> <access key>` reads the secret
  there, as mc documents it);
- the child's environment (`Command::env`, `NAME=value command`);
- a file of mode 0600 that the child reads and the parent removes.

When a value is named like a secret and is not one, the line says so:
`secret-argv: <why>`. A bare marker is not a reason.

# Failure behaviour

The typed capability `quality.secret_argv` reads every Rust source and every shell unit the
tree would commit. It returns one finding for each command line that carries a secret,
naming the file, the line, the value (by name, never its content), the command it reaches,
and this rule. The gate `secret-argv` (`bin/majordomus-cli quality secret-argv`) exits 10
when there is a finding, and 12 when the tree cannot be read. A tree that cannot be read is
never a pass.

# Verification

`test/cases/3996_a_secret_never_travels_on_a_command_line.sh` runs the gate. It must refuse
a Rust fixture with the I2674 shape and a shell fixture that passes a token as an argument,
and pass the same values given on standard input or in the environment. It also runs over
MCP and over this repository's own tree.

The unit tests prove the rest:

- `apps/majordomus-cli/src/quality/secret_argv.rs`: every source and every sink;
- `apps/majordomus-cli/src/mesh/cache_provision.rs`: with a stand-in `mc` that records its
  argv and its standard input, no node's secret is in an argv, and each node's secret is the
  standard input of its own `admin user add`.

The check has known blind spots, which a reviewer covers:

- a secret that arrives in a parameter with a name that does not mark it;
- an argument vector with another name;
- a shell function that passes its argument on;
- the GitHub workflows, which are not shell units.
