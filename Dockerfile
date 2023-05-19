FROM rustlang/rust:nightly-buster AS builder

WORKDIR /usr/src/zauth

RUN cargo install --version=1.4.1 diesel_cli
COPY . .
RUN cargo install --path .

FROM node:14 AS staticbuilder

WORKDIR /usr/src/zauth
COPY . .
RUN npm install
RUN npm run-script build

FROM debian:buster-slim

WORKDIR /usr/src/zauth

RUN apt-get update && apt-get install -y netcat-openbsd sqlite3 libpq-dev libmariadbclient-dev ca-certificates
COPY --from=builder /usr/local/cargo/bin/diesel /usr/local/cargo/bin/zauth /usr/local/bin/
COPY diesel.toml /usr/src/zauth/
COPY migrations/ migrations/
COPY --from=staticbuilder /usr/src/zauth/static static/

ENV ROCKET_ENV production
CMD ["zauth"]
