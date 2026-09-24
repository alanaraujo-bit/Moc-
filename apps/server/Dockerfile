# Mocó sync server. Build from the repository root:
#   docker build -f apps/server/Dockerfile .
FROM rust:1-bookworm AS build
WORKDIR /src
COPY . .
RUN cargo build --release -p moco-server --locked \
 && cp /moco-target/release/moco-server /usr/local/bin/moco-server

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/* \
 && useradd --system --uid 10001 moco
COPY --from=build /usr/local/bin/moco-server /usr/local/bin/moco-server
USER moco
ENV RUST_LOG=info,sqlx=warn
EXPOSE 8080
CMD ["moco-server"]
