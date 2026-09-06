#!/usr/bin/env bash
# sourced by doctor (and by watch through it); guard against re-sourcing
[ -n "${MJ_LIB_deployment:-}" ] && return 0 || MJ_LIB_deployment=1
# deployment — the canonical deployment objects of the layer, read as the contract defines
# them. This file holds the shell half: the contract the section is closed under, decided
# without a Rust toolchain present. The semantic half — a health route no capability
# registers, a package the workspace does not contain, a build input that does not
# resolve — belongs to the executable, which has the registry and the workspace in hand.

# ---------------------------------------------------------------- the doctrine
# majordomus.deployment-contract: a deployment is described once, in the layer, against a
# closed contract; every provider artifact is generated from that description.
mj_validate_deployments() {
  local dir f flat n=0 bad=0 ids="" id
  [ -n "${MJ_DEPLOYMENTS_DIR:-}" ] || return 0    # a layer with no deployments section owes nothing
  dir="$MJ_DEPLOYMENTS_DIR"
  [ -d "$dir" ] || return 0
  [ -f "$MJ_ALLOW_DIR/deployment.txt" ] || {
    mj_doctrine_fail deployment "$(mj_rel "$MJ_ALLOW_DIR")/deployment.txt" \
      "allow-list absent; the schema was not projected" "majordomus generate allow"
    return 0
  }
  for f in "$dir"/*.yaml; do
    [ -f "$f" ] || continue
    n=$((n + 1))
    flat="$(mktemp "${TMPDIR:-/tmp}/mj.dep.XXXXXX")"
    if ! mj_yaml_flatten "$f" > "$flat" 2>/dev/null; then
      mj_doctrine_fail deployment "$(basename "$f")" "does not parse as YAML" "head -n 20 $(mj_rel "$f")"
      bad=1; rm -f "$flat"; continue
    fi

    local unknown; unknown="$(mj_yaml_unknown_keys "$flat" "$MJ_ALLOW_DIR/deployment.txt" || true)"
    [ -n "$unknown" ] && {
      mj_doctrine_fail deployment "$(basename "$f")" \
        "key(s) the contract does not have: $(printf '%s' "$unknown" | tr '\n' ' ' | sed 's/ $//')" \
        "cat $(mj_rel "$MJ_ALLOW_DIR")/deployment.txt"
      bad=1
    }

    [ "$(mj_yget "$flat" schema)" = "deployment/v1" ] || {
      mj_doctrine_fail deployment "$(basename "$f")" \
        "schema is '$(mj_yget "$flat" schema)', and this executable reads deployment/v1" "head -n 5 $(mj_rel "$f")"
      bad=1
    }

    local key
    for key in kind id title application build.package build.binary build.profile \
               listen.port listen.interface health.liveness health.readiness \
               resources.cpu_kind resources.cpus resources.memory_mb \
               machines.count machines.min_running machines.autostart machines.autostop \
               region provider.name; do
      [ -n "$(mj_yget "$flat" "$key")" ] || {
        mj_doctrine_fail deployment "$(basename "$f")" "lacks $key, which every deployment states" "head -n 40 $(mj_rel "$f")"
        bad=1
      }
    done

    # An interface is loopback or every one, and a hosted deployment says which it means:
    # `all` is the intent that replaces suppressing the bind warning.
    case "$(mj_yget "$flat" listen.interface)" in
      loopback|all|"") ;;
      *) mj_doctrine_fail deployment "$(basename "$f")" \
           "listen.interface '$(mj_yget "$flat" listen.interface)' is neither loopback nor all" "grep -n 'interface:' $(mj_rel "$f")"; bad=1 ;;
    esac

    # The process runs as a non-root user, so a privileged port could not be bound.
    local port; port="$(mj_yget "$flat" listen.port)"
    case "$port" in
      ''|*[!0-9]*) ;;
      *) [ "$port" -ge 1024 ] && [ "$port" -le 65535 ] || {
           mj_doctrine_fail deployment "$(basename "$f")" \
             "listen.port $port is outside 1024-65535; the process runs unprivileged" "grep -n 'port:' $(mj_rel "$f")"; bad=1
         } ;;
    esac

    # More machines running than the application has is a bill nobody agreed to.
    local count min; count="$(mj_yget "$flat" machines.count)"; min="$(mj_yget "$flat" machines.min_running)"
    case "$count$min" in
      ''|*[!0-9]*) ;;
      *) [ "$min" -le "$count" ] || {
           mj_doctrine_fail deployment "$(basename "$f")" \
             "machines.min_running $min exceeds machines.count $count" "grep -n -A4 'machines:' $(mj_rel "$f")"; bad=1
         } ;;
    esac

    # A route a platform polls is a path, not a URL: the host is the deployment's, not the
    # object's to state.
    for key in health.liveness health.readiness; do
      case "$(mj_yget "$flat" "$key")" in
        /*|"") ;;
        *) mj_doctrine_fail deployment "$(basename "$f")" \
             "$key '$(mj_yget "$flat" "$key")' is not a path beginning with /" "grep -n -A3 'health:' $(mj_rel "$f")"; bad=1 ;;
      esac
    done

    # No credential belongs in the layer. The contract has no field for one, so this
    # catches a value smuggled into a field that does exist.
    local secret; secret="$(awk -F= '$2 ~ /(^|[^A-Za-z0-9])(fo1_|fm1[ar]_|FlyV1|gh[pousr]_)/ { print $1 }' "$flat" | head -n 3 | tr '\n' ' ')"
    [ -n "$secret" ] && {
      mj_doctrine_fail deployment "$(basename "$f")" "carries what reads as a credential in: ${secret% }" "grep -n 'fo1_\|FlyV1' $(mj_rel "$f")"
      bad=1
    }

    id="$(mj_yget "$flat" id)"
    case " $ids " in
      *" $id "*) mj_doctrine_fail deployment "$id" "two objects claim this identity" "grep -rln 'id: $id' $(mj_rel "$dir")"; bad=1 ;;
      *) ids="$ids $id" ;;
    esac
    rm -f "$flat"
  done
  [ "$n" -gt 0 ] && [ "$bad" = 0 ] \
    && mj_doctrine_ok deployment "$(mj_rel "$dir")/" "$n deployment(s) — every field the contract has, no credential, one identity each" "majordomus doctor"
  return 0
}
