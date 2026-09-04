# The canonical project model: what parses, what is refused, and what is never stored.
#
# The schema rule is the same one policy and profiles live under — a key nobody reads is an
# error, not a comment — so the negative cases here are the point of the file.
. "$ROOT/test/lib.sh"
"$MJ" init >/dev/null

# a repository with no model says so, and says where the model would live
expect_exit 12 "$MJ" plan validate
expect_grep 'no canonical project model'

pj_init
pj_milestone M000
pj_issue I0001 M000
expect_exit 0 "$MJ" plan validate
expect_grep '1 milestone\(s\), 1 issue\(s\), 0 failure\(s\)'

# --- a key nobody reads is an error
cp .ai/repo/project/issues/I0001.yaml /tmp/I0001.keep.$$
printf 'estimate: 3d\n' >> .ai/repo/project/issues/I0001.yaml
expect_exit 10 "$MJ" plan validate
expect_grep 'unknown keys: estimate'
cp /tmp/I0001.keep.$$ .ai/repo/project/issues/I0001.yaml

printf 'burndown: yes\n' >> .ai/repo/project/milestones/M000.yaml
expect_exit 10 "$MJ" plan validate
expect_grep 'unknown keys: burndown'
sed '$d' .ai/repo/project/milestones/M000.yaml > /tmp/m.$$ && mv /tmp/m.$$ .ai/repo/project/milestones/M000.yaml
expect_exit 0 "$MJ" plan validate

# --- status is not a field. Writing one is an unknown key, so no record can contradict the graph.
printf 'status: done\n' >> .ai/repo/project/issues/I0001.yaml
expect_exit 10 "$MJ" plan validate
expect_grep 'unknown keys: status'
cp /tmp/I0001.keep.$$ .ai/repo/project/issues/I0001.yaml

# --- the filename is the id; a record that disagrees with its own filename is refused
cp .ai/repo/project/issues/I0001.yaml .ai/repo/project/issues/I0002.yaml
expect_exit 10 "$MJ" plan validate
expect_grep "declares id 'I0001' but its filename says I0002"
rm .ai/repo/project/issues/I0002.yaml

# --- an issue without acceptance criteria is a placeholder, and placeholders are refused
pj_issue I0003 M000
awk '!/^acceptance_criteria:/ && !/^  - The work is done$/' .ai/repo/project/issues/I0003.yaml > /tmp/i.$$ \
  && mv /tmp/i.$$ .ai/repo/project/issues/I0003.yaml
expect_exit 10 "$MJ" plan validate
expect_grep 'no acceptance criteria'
rm .ai/repo/project/issues/I0003.yaml

# --- an issue naming a milestone that does not exist is refused
pj_issue I0004 M999
expect_exit 10 "$MJ" plan validate
expect_grep 'names milestone M999, which does not exist'
rm .ai/repo/project/issues/I0004.yaml
expect_exit 0 "$MJ" plan validate

# --- a milestone with no issues is reported, because an outcome nobody executes is a warning
pj_milestone M001 1
expect_exit 0 "$MJ" plan validate
expect_grep 'M001 — is reachable and has no issues'

# --- one id, one record: a milestone and an issue may not claim the same id
cp .ai/repo/project/milestones/M001.yaml .ai/repo/project/issues/M001.yaml
sed -i.bak 's/^milestone:.*$//' .ai/repo/project/issues/M001.yaml 2>/dev/null || true
rm -f .ai/repo/project/issues/M001.yaml.bak
expect_exit 10 "$MJ" plan validate
expect_grep 'duplicate id M001'
rm .ai/repo/project/issues/M001.yaml
expect_exit 0 "$MJ" plan validate

# --- the canonical files are never generated: nothing under project/ carries a do-not-edit banner
expect_no_grep 'DO NOT EDIT' .ai/repo/project/project.yaml
rm -f /tmp/I0001.keep.$$
