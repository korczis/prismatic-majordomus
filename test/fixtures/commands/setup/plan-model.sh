# Installed, with a canonical project model carrying one milestone and two issues, the
# second depending on the first. The helpers come from test/lib.sh, which the fixture
# runner has already sourced.
. "$FIXTURE_SETUP/installed.sh"
pj_init
pj_milestone M000
pj_issue I0001 M000
pj_issue I0002 M000 I0001
