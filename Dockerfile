FROM rust:latest AS builder
RUN rustup component add rustfmt