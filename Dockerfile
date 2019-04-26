FROM debian:9

# Add a normal user
RUN groupadd --gid 1000 user \
  && useradd --uid 1000 --gid user --shell /bin/bash --create-home user

# curl and ca-certificates are needed for rustup install.
# the rest is just useful utilities for dev work.
RUN apt-get update && apt-get install --no-install-recommends -y \
  build-essential \
  ca-certificates \
  curl \
  less \
  vim \
  git \
  strace

USER user
WORKDIR /home/user

# TODO Use a pre-made Rust dev image, maybe? It'd build a lot faster.
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs > rustup.sh && chmod +x rustup.sh && ./rustup.sh -y

ENV USER=user
ENV HOME=/home/user
ENV PATH="$HOME/.cargo/bin:$PATH"

# Pre-fetch packages during the image build.
# This is kind of ugly but it works...
RUN cargo init --bin docker-localbind
COPY Cargo.toml docker-localbind/
RUN cd docker-localbind && cargo build

COPY .vimrc .
