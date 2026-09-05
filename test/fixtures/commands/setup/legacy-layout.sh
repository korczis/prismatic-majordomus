# A repository still on the pre-.ai layout: project data under .majordomus/, no .ai/ at all.
# The tool no longer writes that layout, so it is assembled from a fresh installation.
. "$FIXTURE_SETUP/installed.sh"
mkdir -p .majordomus
for s in policy.yaml profiles prompts project; do [ -e ".ai/repo/$s" ] && mv ".ai/repo/$s" ".majordomus/$s"; done
mv .ai/local/state .majordomus/state
rm -rf .ai
sed -i.bak '/^\.ai\/local\/$/d' .gitignore && rm -f .gitignore.bak
git add -A && git commit -qm legacy
