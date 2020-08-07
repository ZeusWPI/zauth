FROM rustlang/rust:nightly-buster

WORKDIR /usr/src/zauth
COPY . .

RUN apt-get update && apt-get install -y netcat

RUN cargo install diesel_cli
RUN cargo install --path .

ENV ROCKET_ENV production
CMD ["zauth"]