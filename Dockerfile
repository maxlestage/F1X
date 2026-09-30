# --- Build ---
FROM rust:1-slim-bookworm AS build
WORKDIR /src
COPY web/Cargo.toml web/Cargo.lock ./
# Pre-build dependencies so code-only changes rebuild fast.
RUN mkdir src && echo 'fn main() {}' > src/main.rs && cargo build --release && rm -rf src
COPY web/src ./src
COPY web/static ./static
RUN touch src/main.rs && cargo build --release

# --- Runtime ---
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/f1x-web /usr/local/bin/f1x-web
RUN useradd --create-home app
USER app
ENV PORT=3000
CMD ["f1x-web"]
