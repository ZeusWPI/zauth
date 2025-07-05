FROM rust:bookworm AS builder

WORKDIR /usr/src/zauth

RUN cargo install diesel_cli
COPY . .
RUN cargo install --locked --path .

FROM node:22 AS staticbuilder

WORKDIR /usr/src/zauth
COPY . .
RUN npm install
RUN npm run-script build

FROM debian:bookworm-slim

WORKDIR /usr/src/zauth

RUN apt-get update && apt-get install -y netcat-openbsd sqlite3 libpq-dev libmariadb-dev ca-certificates
COPY --from=builder /usr/local/cargo/bin/diesel /usr/local/cargo/bin/zauth /usr/local/bin/
COPY diesel.toml /usr/src/zauth/
COPY migrations/ migrations/
COPY --from=staticbuilder /usr/src/zauth/static static/

ENV ROCKET_ENV production
CMD ["zauth"]
