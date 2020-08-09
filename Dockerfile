FROM rustlang/rust:nightly-alpine AS builder

WORKDIR /usr/src/zauth

RUN apk add --no-cache musl-dev mariadb-connector-c-dev postgresql-dev sqlite-dev

RUN cargo install diesel_cli
COPY . .
RUN cargo install --path .

FROM alpine

WORKDIR /usr/src/zauth

RUN apk add --no-cache netcat-openbsd libpq
COPY --from=builder /usr/local/cargo/bin/diesel /usr/local/cargo/bin/zauth /usr/local/bin/
COPY Rocket.toml diesel.toml /usr/src/zauth/
COPY migrations/ migrations/
COPY docker_misc/ docker_misc/
COPY static/ static/

ENV ROCKET_ENV production
CMD ["zauth"]
