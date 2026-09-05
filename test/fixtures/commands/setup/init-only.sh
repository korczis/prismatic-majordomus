# Installed, but the provider instruction files have not been generated yet.
"$MJ" init >/dev/null
mkdir -p lib && echo a > lib/a && git add . && git commit -qm base
