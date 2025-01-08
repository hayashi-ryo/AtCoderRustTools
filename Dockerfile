FROM rust
RUN rustup install 1.70.0 && rustup override set 1.70.0 && rustup component add --toolchain 1.70.0-aarch64-unknown-linux-gnu rustfmt