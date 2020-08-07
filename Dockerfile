FROM rustlang/rust:nightly-buster

WORKDIR /usr/src/zauth

RUN apt-get update && apt-get install -y netcat

RUN cargo install diesel_cli
COPY . .
RUN cargo install --path .

ENV ROCKET_ENV production
CMD ["zauth"]