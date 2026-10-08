# syntax=docker/dockerfile:1

FROM node:22-bookworm-slim AS assets
WORKDIR /build
COPY package.json package-lock.json ./
RUN npm ci
COPY frontend ./frontend
COPY scripts/build-assets.mjs ./scripts/build-assets.mjs
RUN npm run build

FROM rust:bookworm AS rust-build
WORKDIR /build
COPY Cargo.toml Cargo.lock build.rs ./
COPY src ./src
COPY migrations ./migrations
RUN cargo build --locked --release --bins

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates sqlite3 \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /var/data
WORKDIR /app
COPY --from=rust-build /build/target/release/jobtowel /usr/local/bin/jobtowel
COPY --from=rust-build /build/target/release/migrate /usr/local/bin/migrate
COPY --from=assets /build/static/assets ./static/assets
COPY --chmod=755 scripts/docker-entrypoint.sh /usr/local/bin/docker-entrypoint

ENV DATABASE_URL=sqlite:///var/data/jobtowel.db \
    ASSET_DIR=/app/static/assets \
    SESSION_COOKIE_SECURE=true \
    RUST_LOG=jobtowel=info,tower_http=info
EXPOSE 10000
ENTRYPOINT ["docker-entrypoint"]
CMD ["jobtowel"]
