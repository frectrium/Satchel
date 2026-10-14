# One image holds every Satchel binary; docker-compose picks which to run.

FROM rust:1.99-slim-bookworm AS builder
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --workspace --locked \
    && mkdir -p /out \
    && cp target/release/satchel-api target/release/satchel-index \
          target/release/satchel-node target/release/satchel-verifier /out/

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --system --create-home satchel \
    && mkdir -p /data && chown satchel /data
COPY --from=builder /out/ /usr/local/bin/
USER satchel
VOLUME ["/data"]
ENV RUST_LOG=info
CMD ["satchel-api"]
