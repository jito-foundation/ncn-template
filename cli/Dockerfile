##### Builder image
FROM rust:1.80.0-slim-bullseye as builder

RUN apt-get update && apt-get install -y \
    libudev-dev \
    clang \
    pkg-config \
    libssl-dev \
    build-essential \
    cmake \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/* \
    && update-ca-certificates

WORKDIR /usr/src/app

COPY . .

RUN cargo build --release --bin ncn-program-cli
RUN cargo install --path ./cli --bin ncn-program-cli --locked

##### Final image
FROM debian:bullseye-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl1.1 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /usr/src/app/target/release/ncn-program-cli /usr/local/bin/ncn-program-cli

ENTRYPOINT ["ncn-program-cli", "keeper"]