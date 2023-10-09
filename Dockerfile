FROM rust:1.73 AS builder
WORKDIR /app

RUN rustup component add rustfmt
RUN apt-get update && apt-get install -y protobuf-compiler

COPY Cargo.* ./
COPY ./crates ./crates

RUN cargo test -j$(nproc) --workspace
RUN cargo build -j$(nproc) --workspace --release
RUN cargo install -j$(nproc) --path ./crates/database
RUN cargo install -j$(nproc) --path ./crates/push-notifications-api
RUN cargo install -j$(nproc) --path ./crates/push-notifications-processor-orders
RUN cargo install -j$(nproc) --path ./crates/push-notifications-processor-prices
RUN cargo install -j$(nproc) --path ./crates/push-notifications-sender


FROM debian:12 as runtime
WORKDIR /app

RUN apt-get update && apt-get install -y curl openssl libssl-dev libpq-dev
RUN /usr/sbin/update-ca-certificates

COPY --from=builder /usr/local/cargo/bin/* ./

CMD ["/app/api"]
