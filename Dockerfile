FROM alpine:3.19 AS builder
WORKDIR /app

RUN apk add --no-cache curl bash build-base musl-dev && \
    curl -fsSL https://mise.run | sh

ENV PATH="/root/.local/bin:$PATH"

COPY mise.toml .
RUN mise trust . && mise install

COPY Cargo.toml Cargo.lock* ./

ENV RUNNING_IN_DOCKER=true

RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    mise exec -- cargo build --release

RUN rm -rf src

COPY src ./src
COPY templates ./templates

RUN touch src/main.rs && \
    mise exec -- cargo build --release

FROM alpine:latest
WORKDIR /root/

COPY --from=builder /app/target/release/mcmanagement .
RUN mkdir /app

EXPOSE 8080
CMD ["./mcmanagement"]