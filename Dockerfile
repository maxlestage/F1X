# --- Build : serveur axum + frontend active (WebAssembly, compilé par server/build.rs) ---
FROM rust:1-slim-trixie AS build
RUN rustup target add wasm32-unknown-unknown
WORKDIR /src
COPY web/ ./
RUN cargo build --release --locked

# --- Runtime ---
FROM debian:trixie-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=build /src/target/release/f1x-web /usr/local/bin/f1x-web
RUN useradd --create-home app
USER app
ENV PORT=3000
CMD ["f1x-web"]
