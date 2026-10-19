# The archive's name and its root directory for one target, read from the build matrix
# generated from the distribution model (docs/generated/distribution-matrix.json), with the
# tag the matrix leaves as {tag} filled in. Two lines, name first, or nothing for a target the
# matrix does not publish. scripts/release-package and scripts/release-verify read it with
# --names-from; `majordomus distribution artifact` derives the same two names from the model,
# and the plan job of the release workflow holds the matrix to the model before any build.
#
#   jq -r --arg target <rust target> --arg tag <vX.Y.Z> -f scripts/lib/distribution-names.jq <matrix>
[.include[] | select(.target == $target)][0] // empty
| (.artifact, .root)
| gsub("\\{tag\\}"; $tag)
