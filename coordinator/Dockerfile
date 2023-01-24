FROM ubuntu:22.04

LABEL "org.opencontainers.image.source"="https://github.com/get10101/10101"
LABEL "org.opencontainers.image.authors"="hello@10101.finance"

ARG BINARY=target/release/coordinator

USER 1000

COPY $BINARY /usr/bin/coordinator

ENTRYPOINT ["/usr/bin/coordinator"]
