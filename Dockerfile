FROM rust:1.96-slim-bookworm AS builder
WORKDIR /app
COPY . .
RUN cargo build --release && strip target/release/sparrow

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates git && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/sparrow /usr/local/bin/sparrow
RUN mkdir -p /root/.config/sparrow /root/.local/state/sparrow
VOLUME ["/workspace"]
WORKDIR /workspace
ENTRYPOINT ["sparrow"]
CMD ["--help"]
