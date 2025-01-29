FROM rust:1.84-slim-bookworm AS builder

# use rustfmt
RUN rustup component add rustfmt

# intall for package for build mold
RUN apt-get update && \
  apt-get install -y \
  build-essential \
  git \
  clang \
  lld \
  cmake \
  libssl-dev \
  libxxhash-dev \
  zlib1g-dev \
  pkg-config

# install mold
ENV mold_version=v1.1
RUN git clone --branch "$mold_version" --depth 1 https://github.com/rui314/mold.git && \
  cd mold && \
  make -j$(nproc) CXX=clang++ && \
  make install && \
  mv /mold/mold /usr/bin/mold && \
  mv /mold/mold-wrapper.so /usr/bin/mold-wrapper.so && \
  make clean