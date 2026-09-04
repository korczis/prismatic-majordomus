# Installed and generated, then the policy was edited without regenerating the projections.
. "$FIXTURE_SETUP/installed.sh"
printf '\nledger:\n  retention_max_lines: 4000\n' >> .majordomus/policy.yaml
