FROM rust:1.65 AS builder
WORKDIR /app

RUN rustup component add rustfmt
RUN apt-get update && apt-get install -y protobuf-compiler

COPY Cargo.* ./
COPY ./src ./src
COPY ./migrations ./migrations

RUN cargo install -j4 --path .


FROM debian:11 as runtime
WORKDIR /app

RUN apt-get update && apt-get install -y curl openssl libssl-dev libpq-dev
RUN /usr/sbin/update-ca-certificates

COPY --from=builder /usr/local/cargo/bin/* ./

CMD ["/app/api"]
