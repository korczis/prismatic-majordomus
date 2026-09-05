# The native command line, projected from the executable's own document
# (docs/generated/cli.json) into the shape the site renders: one flat list of commands, each
# carrying its route, its ancestors, its children and its examples. The tree is flattened
# here and nowhere else, because Tera has no recursion; the routes are the executable's own
# and are never recomputed.
#
#   jq -S -f scripts/lib/cli-site.jq docs/generated/cli.json
def node($ancestors):
  . as $c
  | {
      name: ($c.path | last),
      command: ($c.path | join(" ")),
      path: $c.path,
      route: $c.route,
      depth: (($c.path | length) - 1),
      parent_route: (if ($ancestors | length) > 0 then ($ancestors | last | .route) else null end),
      ancestors: $ancestors,
      executable: $c.executable,
      about: $c.about,
      long_about: ($c.long_about // null),
      usage: $c.usage,
      args: ($c.args // []),
      examples: ($c.examples // []),
      children: [ $c.subcommands[]? | { name: (.path | last), command: (.path | join(" ")), route: .route, about: .about } ]
    } as $self
  | [ $self ]
    + ( [ $c.subcommands[]? | node($ancestors + [ { name: $self.name, command: $self.command, route: $self.route } ]) ] | add // [] );

{ schema: 1,
  source: "docs/generated/cli.json",
  declaration: .source,
  generator_version: .version,
  route_prefix: .route_prefix,
  commands: (.cli | node([])) }
| .counts = { commands: (.commands | length),
              runnable: ([ .commands[] | select(.executable) ] | length),
              examples: ([ .commands[].examples[] ] | length),
              arguments: ([ .commands[].args[] ] | length) }
