#compdef majordomus majordomus-cli
# Majordomus completion for zsh — a generic adapter, installed once, never regenerated.
#
# It contains no command, no flag and no value of any repository. Its whole job is to say
# what has been typed and where the cursor is, and to render what the executable answers.
# Everything offered comes from `majordomus completion query`, which reads the canonical
# command graph of the repository the shell is standing in. A new command, a new workflow
# or a new accepted value therefore appears without this file changing.

_majordomus_render() {
  local -a raw parts
  local line value description
  local -a values descriptions
  raw=("${(@f)$(${1} completion query --surface ${2} --cursor $((CURRENT - 1)) --format shell -- "${@:3}" 2>/dev/null)}")
  for line in "${raw[@]}"; do
    [[ -z "$line" ]] && continue
    value="${line%%$'\t'*}"
    description="${line#*$'\t'}"
    [[ "$description" == "$line" ]] && description=""
    if [[ "$value" == "<path>" ]]; then
      _files
      continue
    fi
    values+=("$value")
    descriptions+=("${value}:${description}")
  done
  if (( ${#values} )); then
    _describe -t majordomus 'majordomus' descriptions
  fi
}

_majordomus() {
  _majordomus_render "${words[1]}" cli "${(@)words[2,-1]}"
}

# `just` in a Majordomus repository: the recipes are the bridge projection of the same
# graph, so the same engine answers, addressed by the `just` surface.
_majordomus_just() {
  local exe
  exe="$(command -v majordomus 2>/dev/null)"
  [[ -x ./bin/majordomus-cli ]] && exe=./bin/majordomus-cli
  [[ -n "$exe" ]] || return 1
  _majordomus_render "$exe" just "${(@)words[2,-1]}"
}

compdef _majordomus majordomus majordomus-cli
