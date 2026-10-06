# majordomus-covers: doctor update
# A layer written for a newer tool than the one installed is refused, and the refusal names
# both versions (I1980).
#
# A newer layer is the one case where the tool cannot judge anything: it does not know the
# rules the layer was written for. doctor fails naming the upgrade; update refuses to bring
# it "up" to an older version; and a manifest this executable cannot read at all, because a
# newer tool added a key, is refused with one sentence naming both versions rather than with
# the unknown key it happens to meet first.
. "$ROOT/test/lib.sh"

"$MJ" init >/dev/null
"$MJ" update >/dev/null
tool="$("$MJ" version | awk '{print $2}')"
major="${tool%%.*}"
future="$((major + 1)).0.0"
stamp() { sed -i.bak "s/^written_for: .*/written_for: \"$1\"/" .ai/manifest.yaml && rm -f .ai/manifest.yaml.bak; }

echo "    doctor fails a layer written for a newer tool, naming both versions and the upgrade"
stamp "$future"
expect_exit 10 "$MJ" doctor
expect_grep "FAIL layout +\.ai/manifest\.yaml — written for majordomus $future, newer than this executable \($tool\): upgrade the tool"

echo "    update refuses it and leaves the layer as it was"
before="$(cat .ai/manifest.yaml)"
expect_exit 15 "$MJ" update
expect_grep "was written for majordomus $future, newer than this executable \($tool\); upgrade the tool"
[ "$(cat .ai/manifest.yaml)" = "$before" ] || { echo "    update rewrote a newer layer"; exit 1; }

echo "    a manifest from the future, with a key this executable does not know: one message"
printf 'future_section: 1\n' >> .ai/manifest.yaml
for cmd in doctor check "plan status"; do
  # shellcheck disable=SC2086  # the command's words
  expect_exit 10 "$MJ" $cmd
  expect_grep "written for majordomus $future, newer than this executable \($tool\); upgrade the tool"
  expect_no_grep 'unknown key'
done

echo "    the same manifest named for an older tool is refused for its key, as before"
stamp "0.0.0"
expect_exit 10 "$MJ" doctor
expect_grep 'unknown key\(s\): future_section'
