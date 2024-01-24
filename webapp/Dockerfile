FROM ubuntu:22.04

LABEL "org.opencontainers.image.source"="https://github.com/get10101/10101"
LABEL "org.opencontainers.image.authors"="hello@10101.finance"

ARG BINARY=target/release/webapp

RUN apt-get update && \
    apt-get install libpq5 -y && \
    apt-get install sqlite3 -y && \
    apt-get install ca-certificates openssl -y

RUN mkdir -p webapp/certs

RUN cd webapp/certs && \
    openssl req -x509 -newkey rsa:4096 -keyout key.pem -out cert.pem -sha256 -days 3650 -nodes -subj "/C=XX/ST=StateName/L=CityName/O=CompanyName/OU=CompanySectionName/CN=CommonNameOrHostname"

RUN chmod a+r -R webapp/certs

USER 1000

COPY $BINARY /usr/bin/webapp

ENTRYPOINT ["/usr/bin/webapp"]
