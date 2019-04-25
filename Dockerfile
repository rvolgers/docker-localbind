FROM debian:9

# Add a normal user
RUN groupadd --gid 1000 user \
  && useradd --uid 1000 --gid user --shell /bin/bash --create-home user

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

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs > rustup.sh && chmod +x rustup.sh && ./rustup.sh -y

ENV USER=user
ENV HOME=/home/user
ENV PATH="$HOME/.cargo/bin:$PATH"

RUN cargo init --bin docker-localbind
COPY Cargo.toml docker-localbind/
RUN cd docker-localbind && cargo build

COPY .vimrc .
