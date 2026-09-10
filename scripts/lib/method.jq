# method.jq — the governance method as one dataset: the layers an invariant descends through,
# the counts at each of them, one worked chain from a principle to a failing exit code, and the
# four axes a piece of work is described on.
#
# The words in `layers` and `axes` are vocabulary — the question a layer answers, how long an
# answer at that layer lives, what a reader should not confuse with what. Every number, name,
# route, command and file beside them is a join over data the executable and the rule package
# already own, so a rule added tomorrow, a validator removed, or a capability that stops being
# exposed on an interface moves this page with no one editing it.
#
# Input is the capability registry manifest (docs/generated/registry.json); doctrines,
# lifecycle, policy and profiles arrive as slurped files, the skeleton's workflow count as
# $workflows.
#
#   jq -S --slurpfile d ... -f scripts/lib/method.jq docs/generated/registry.json
def has_test: (.test // "") | (. != "" and . != "-");

.capabilities as $caps
| $d[0].doctrines as $rules
| $l[0].principles as $principles
| $pol[0] as $policy
| $prof[0].profiles as $profiles
| ([$rules[] | select(.principle != "") | .principle] | unique) as $principles_enforced
| ([$rules[] | .validator] | unique) as $validators
| ([$rules[] | .enforced_by[]] | unique | sort) as $commands
| ([$rules[] | select(has_test) | .test] | unique) as $tests
| ([$rules[] | select(.class == "blocking")] | length) as $blocking
| ( $rules
    | map(select(has_test))
    | sort_by([-( .claims | length ), .slug])
    | .[0] ) as $ex
| { cli: "Command line", http: "HTTP + OpenAPI", mcp: "MCP tools and resources" } as $surface_labels
| ( [ $caps[] | .exposure | keys[] ] | unique | sort
    | map(. as $s | { id: $s, label: ($surface_labels[$s] // $s),
                      count: ([$caps[] | select(.exposure[$s])] | length) }) ) as $surfaces
| ( [
      { level: "Doctrine", question: "What must remain true?", lifetime: "years",
        artifact: "a principle in the portable rule package; technology-neutral, and checked by nothing",
        count: ($principles | length), unit: "principles", route: "/doctrines/" },
      { level: "Policy", question: "How is the invariant applied here?", lifetime: "months to years",
        artifact: "one policy file, projected into the instruction file of every worker that opens the repository",
        count: ($policy.projections | length), unit: "projections", route: "/policy/" },
      { level: "Rule", question: "What exactly must, or must not, be?", lifetime: "months",
        artifact: "a rule file with a class, a validator, the commands that run it and the test that proves it",
        count: ($rules | length), unit: "rules", route: "/doctrines/" },
      { level: "Enforcement", question: "How does a machine decide it?", lifetime: "the implementation that answers it",
        artifact: "a validator, dispatched from a command, wired to a git hook and to CI",
        count: ($validators | length), unit: "validators", route: "/contract/" },
      { level: "Diagnostic", question: "What broke, and what fixes it?", lifetime: "one run",
        artifact: "a finding naming the rule, the file, the reason and the exit code the command leaves with",
        count: ($commands | length), unit: "reporting commands", route: "/evidence/" }
  ] ) as $layers
| ( [
      { axis: "Profile", answers: "with how much context, effort and ceremony",
        artifact: "one file per working mode under the layer",
        count: ($profiles | length), unit: "profiles", route: "/profiles/",
        note: "It declares a capability class and an effort, never a vendor or a model name." },
      { axis: "Workflow", answers: "in what order, and what must exist before it may end",
        artifact: "a sequence a worker follows, seeded into every repository",
        count: $workflows, unit: "workflows", route: "/getting-started/",
        note: "It names capabilities and phases, never the backend that answers them." },
      { axis: "Surface", answers: "where the same capability can be reached",
        artifact: "not a file anyone writes: the exposure a capability declares once",
        count: ($surfaces | length), unit: "interfaces", route: "/registry/capabilities/",
        note: "An interface is a projection of the declaration; a projection that disagrees with it fails the build." },
      { axis: "Model", answers: "what actually executes the work",
        artifact: "chosen per run, outside the repository",
        count: 0, unit: "declared anywhere in the layer", route: "/limitations/",
        note: "Majordomus never invokes one. Continuity lives in the records, so the model can change mid-task without the work losing its state." }
  ] ) as $axes
| {
    schema: 1,
    source: "share/standard/majordomus/, .ai/repo/policy.yaml, docs/generated/registry.json",

    # One row per layer an invariant descends through. `count` is what this repository holds
    # at that layer today; `route` is the page that lists it.
    layers: $layers,

    governance: {
      principles_declared: ($principles | length),
      principles_enforced: ($principles_enforced | length),
      principles_without_rule: ($principles - $principles_enforced),
      rules: ($rules | length),
      blocking: $blocking,
      advisory: (($rules | length) - $blocking),
      validators: ($validators | length),
      commands: $commands,
      tests: ($tests | length),
      rules_without_principle: [$rules[] | select(.principle == "") | { slug: .slug, title: .title, route: ("/doctrines/" + .slug + "/") }],
      rules_without_test: [$rules[] | select(has_test | not) | { slug: .slug, route: ("/doctrines/" + .slug + "/") }],
      exit_codes: ([$rules[] | .exit_code] | unique | sort)
    },

    # The worked chain: the rule that carries the most claims of any rule with a test, which
    # is the one with the most of the repository standing on it. Chosen by the data, not named.
    example: {
      principle: $ex.principle,
      slug: $ex.slug,
      title: $ex.title,
      statement: $ex.statement,
      class: $ex.class,
      validator: $ex.validator_function,
      defined_in: $ex.defined_in,
      enforced_by: $ex.enforced_by,
      exit_code: $ex.exit_code,
      test: $ex.test,
      claims: ($ex.claims | length),
      source: $ex.source,
      route: ("/doctrines/" + $ex.slug + "/")
    },

    # Four axes, deliberately not one. A profile is not a workflow, a workflow is not an
    # interface, and none of the three is a model.
    axes: $axes,

    exposure: {
      capabilities: ($caps | length),
      modules: ([$caps[] | .module] | unique | length),
      surfaces: $surfaces,
      combinations: (
        [ $caps[] | (.exposure | keys | sort | join(" + ")) ]
        | group_by(.) | map({ surfaces: .[0], count: length }) | sort_by(-.count) ),
      by_module: (
        [ $caps[]
          | { module: .module, cli: (if .exposure.cli then 1 else 0 end),
              http: (if .exposure.http then 1 else 0 end),
              mcp: (if .exposure.mcp then 1 else 0 end) } ]
        | group_by(.module)
        | map({ module: .[0].module, capabilities: length,
                cli: (map(.cli) | add), http: (map(.http) | add), mcp: (map(.mcp) | add) })
        | sort_by([-.capabilities, .module]) )
    },

    # The two Mermaid sources of the method, drawn from the fields above and nothing else, so
    # the picture and the table beside it cannot drift apart. Only the shape is written here.
    diagrams: {
      governance: (
        [ "flowchart TD" ]
        + ( $layers | to_entries | map("    L" + (.key | tostring) + "[\"" + .value.level
              + "<br/><small>" + (.value.count | tostring) + " " + .value.unit + " · " + .value.lifetime + "</small>\"]") )
        + [ "    " + ( $layers | to_entries | map("L" + (.key | tostring)) | join(" --> ") ) ]
        + [ "    L" + (($layers | length) - 1 | tostring) + " -. \"exit "
            + ( [$rules[] | select(.class == "blocking") | .exit_code] | unique | map(tostring) | join(", ") )
            + " · the commit does not land\" .-> L2" ]
        | join("\n") ),
      axes: (
        ( $axes | map(select(.axis != "Model" and .axis != "Surface")) ) as $flow
        | [ "flowchart TD", "    I[\"Intent<br/><small>one task, started once</small>\"]" ]
        + ( $flow | to_entries | map("    A" + (.key | tostring) + "[\"" + .value.axis
              + "<br/><small>" + (.value.count | tostring) + " " + .value.unit + "</small>\"]") )
        + [ "    C[\"Capability<br/><small>" + ($caps | length | tostring) + " · declared once</small>\"]" ]
        + [ "    I --> " + ( $flow | to_entries | map("A" + (.key | tostring)) | join(" --> ") ) + " --> C" ]
        + ( $surfaces | to_entries | map("    S" + (.key | tostring) + "[\"" + .value.label
              + "<br/><small>" + (.value.count | tostring) + " of " + ($caps | length | tostring) + "</small>\"]\n    C --> S" + (.key | tostring)) )
        + [ "    M([\"Model<br/><small>chosen per run · named in no file of the layer</small>\"])",
            "    A" + (($flow | length) - 1 | tostring) + " -. \"executed by\" .-> M" ]
        | join("\n") )
    },

    # What the profiles declare instead of a model. Read off the profile files themselves.
    profile_fields: {
      capability_classes: ([$profiles[] | .capability] | unique | sort),
      efforts: ([$profiles[] | .effort] | unique | sort),
      output_contract: ([$profiles[] | .output_contract[]] | unique | sort)
    }
  }
