FROM docker.io/library/debian:trixie-slim AS base

RUN apt-get update && apt-get install -y --no-install-recommends \
    curl \
    ca-certificates \
    git \
    build-essential \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

RUN curl https://mise.jdx.dev/install.sh | sh
ENV PATH="/root/.local/bin:$PATH"

WORKDIR /app

COPY mise.toml ./
COPY mise.lock ./

RUN mise trust

ENV MISE_YES=1
RUN mise install

ENV PATH="/root/.local/share/mise/shims:$PATH"

RUN cargo install cargo-chef

FROM base AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

FROM base AS builder
COPY --from=planner /app/recipe.json recipe.json

RUN cargo chef cook --release --recipe-path recipe.json

ENV PATH="/.cargo/bin:$PATH"

RUN curl -L --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/cargo-bins/cargo-binstall/main/install-from-binstall-release.sh | bash
RUN cargo binstall dioxus-cli --root /.cargo -y --force


ENV RUNNING_IN_DOCKER="true"

COPY . .
RUN dx bundle --package web --release

FROM docker.io/library/debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/dx/web/release/web/ /usr/local/app

ENV PORT=8080
ENV IP=0.0.0.0
EXPOSE 8080

WORKDIR /usr/local/app

ENTRYPOINT [ "/usr/local/app/server" ]