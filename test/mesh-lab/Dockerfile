# One node of the mesh lab: a Linux machine with its own filesystem, its own node key
# (XDG_STATE_HOME), its own checkout and its own network interface on the lab's bridge.
# The executable and the share directory are copied in from the build; nothing of the
# developer's machine is mounted into a running node. Ubuntu 24.04, so that an executable
# built on CI's ubuntu-latest runner and one built in rust:*-bookworm (older glibc) both run.
FROM ubuntu:24.04
RUN apt-get update \
 && apt-get install -y --no-install-recommends git ca-certificates curl jq iproute2 \
 && rm -rf /var/lib/apt/lists/*
COPY majordomus /usr/local/bin/majordomus
COPY share /opt/majordomus/share
COPY repo.tar /lab/repo.tar
COPY node-entrypoint /usr/local/bin/node-entrypoint
ENV MAJORDOMUS_SHARE=/opt/majordomus/share \
    XDG_STATE_HOME=/state \
    MAJORDOMUS_LOG=info
EXPOSE 8741/tcp 7741/udp
ENTRYPOINT ["/usr/local/bin/node-entrypoint"]
