#! /bin/zsh

cargo b
./target/debug/ncn-program-shank-cli && yarn install && yarn generate-clients && cargo b
cargo-build-sbf
cargo fmt
