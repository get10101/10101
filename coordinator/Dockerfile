FROM ubuntu:22.04

LABEL "org.opencontainers.image.source"="https://github.com/get10101/10101"
LABEL "org.opencontainers.image.authors"="hello@10101.finance"

ARG BINARY=target/release/coordinator

# todo: check why sqlite3 is needed as the coordinator only depends on postgres.
RUN apt-get update && \
    apt-get install libpq5 -y && \
    apt-get install sqlite3 -y && \
    apt-get install ca-certificates -y

USER 1000

COPY $BINARY /usr/bin/coordinator

ENTRYPOINT ["/usr/bin/coordinator"]
