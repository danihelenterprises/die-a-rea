# syntax=docker/dockerfile:1
#
# die-a-rea container toolchain.
#
# Targets:
#   build    - compiles the workspace (release)               [intermediate]
#   runtime  - minimal image: the CLI + the JS adapter        [default]
#   dev      - Rust + Node + clippy/rustfmt for building/testing
#
# Build from the repository root:
#   docker build -f docker/Dockerfile --target runtime -t die-a-rea:runtime .
#   docker build -f docker/Dockerfile --target dev     -t die-a-rea:dev .

# ── build ───────────────────────────────────────────────────────────────────
FROM rust:bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --release --locked --workspace

# ── dev ─────────────────────────────────────────────────────────────────────
FROM rust:bookworm AS dev
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates curl gnupg git bash \
 && rm -rf /var/lib/apt/lists/*
RUN curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
 && apt-get install -y --no-install-recommends nodejs \
 && rm -rf /var/lib/apt/lists/* \
 && npm install --global javascript-obfuscator@^5
ENV NODE_PATH=/usr/lib/node_modules
COPY --from=build /src/target/release/die-a-rea /usr/local/bin/die-a-rea
COPY adapters /opt/die-a-rea/adapters
ENV DIREA_JS_ADAPTER=/opt/die-a-rea/adapters/js/direa-js
COPY docker/entrypoint.sh /usr/local/bin/die-a-rea-entrypoint
RUN chmod +x /usr/local/bin/die-a-rea-entrypoint \
 && ln -sf /opt/die-a-rea/adapters/js/direa-js /usr/local/bin/direa-js
WORKDIR /work
ENTRYPOINT ["/usr/local/bin/die-a-rea-entrypoint"]
CMD ["help"]

# ── runtime ─────────────────────────────────────────────────────────────────
FROM node:22-bookworm-slim AS runtime
RUN npm install --global javascript-obfuscator@^5
ENV NODE_PATH=/usr/local/lib/node_modules
COPY --from=build /src/target/release/die-a-rea /usr/local/bin/die-a-rea
COPY adapters /opt/die-a-rea/adapters
RUN chmod +x /opt/die-a-rea/adapters/js/direa-js \
 && ln -sf /opt/die-a-rea/adapters/js/direa-js /usr/local/bin/direa-js
ENV DIREA_JS_ADAPTER=/opt/die-a-rea/adapters/js/direa-js
COPY docker/entrypoint.sh /usr/local/bin/die-a-rea-entrypoint
RUN chmod +x /usr/local/bin/die-a-rea-entrypoint
WORKDIR /work
ENTRYPOINT ["/usr/local/bin/die-a-rea-entrypoint"]
CMD ["help"]
