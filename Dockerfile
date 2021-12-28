FROM rust:latest as rust
RUN apt-get update && apt-get install -y git pkg-config libssl-dev
WORKDIR /src
RUN mkdir src && echo "fn main() {}" > src/main.rs
COPY Cargo.toml .
RUN sed -i '/.*build.rs.*/d' Cargo.toml
COPY Cargo.lock .
COPY migrations /src/migrations
COPY sqlx-data.json /src/
COPY src/tests-migrate.rs /src/src/tests-migrate.rs
COPY src/settings.rs /src/src/settings.rs
RUN cargo --version
RUN cargo build --release
COPY . /src
RUN cargo build --release 

FROM debian:bullseye
LABEL org.opencontainers.image.source https://github.com/kavasam/armory
RUN useradd -ms /bin/bash -u 1001 kavasam
WORKDIR /home/kavasam
COPY --from=rust /src/target/release/armory /usr/local/bin/
COPY --from=rust /src/config/default.toml /etc/kavasam/config.toml
USER kavasam
CMD [ "/usr/local/bin/armory" ]
