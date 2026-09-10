# Majordomus completion for bash — a generic adapter, installed once, never regenerated.
#
# It contains no command, no flag and no value of any repository; everything offered comes
# from `majordomus completion query` against the canonical command graph of the repository
# the shell is standing in.

_majordomus_complete() {
  local surface="$1" exe="$2"
  local -a words_without_program
  local cursor line value
  words_without_program=("${COMP_WORDS[@]:1}")
  cursor=$((COMP_CWORD - 1))
  COMPREPLY=()
  while IFS=$'\t' read -r value _; do
    [[ -z "$value" ]] && continue
    if [[ "$value" == "<path>" ]]; then
      while IFS= read -r line; do COMPREPLY+=("$line"); done < <(compgen -f -- "${COMP_WORDS[COMP_CWORD]}")
      continue
    fi
    COMPREPLY+=("$value")
  done < <("$exe" completion query --surface "$surface" --cursor "$cursor" --format shell -- "${words_without_program[@]}" 2>/dev/null)
}

_majordomus() { _majordomus_complete cli "${COMP_WORDS[0]}"; }

_majordomus_just() {
  local exe
  exe="$(command -v majordomus 2>/dev/null)"
  [[ -x ./bin/majordomus-cli ]] && exe=./bin/majordomus-cli
  [[ -n "$exe" ]] || return 1
  _majordomus_complete just "$exe"
}

complete -o nosort -F _majordomus majordomus majordomus-cli
