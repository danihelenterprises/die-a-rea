# syntax=docker/dockerfile:1
#
# The test bowl — on-demand OSS evaluation image (B-052).
#
# Fetches pinned third-party release artifacts across every ecosystem die-a-rea
# supports, runs plan/build/verify on them, and writes logs + hardened artifacts
# to a mounted /testbowl-out. Prefer the top-level `./testbowl` launcher. To build
# and run directly from the repository root:
#
#   docker build -f eval/Dockerfile -t die-a-rea-testbowl .
#   docker run --rm -v "$PWD/testbowl-out:/testbowl-out" die-a-rea-testbowl all

# ── build: compile the CLI ───────────────────────────────────────────────────
FROM rust:bookworm AS build
WORKDIR /src
COPY Cargo.toml Cargo.lock ./
COPY crates ./crates
RUN cargo build --release --locked -p direa-cli

# ── eval runtime ─────────────────────────────────────────────────────────────
FROM node:22-bookworm-slim
RUN apt-get update \
 && apt-get install -y --no-install-recommends \
      ca-certificates curl bash git tar unzip xz-utils binutils file \
 && rm -rf /var/lib/apt/lists/*
RUN npm install --global javascript-obfuscator@^5 @electron/asar
ENV NODE_PATH=/usr/local/lib/node_modules

COPY --from=build /src/target/release/die-a-rea /usr/local/bin/die-a-rea
COPY adapters /opt/die-a-rea/adapters
RUN chmod +x /opt/die-a-rea/adapters/js/direa-js \
 && ln -sf /opt/die-a-rea/adapters/js/direa-js /usr/local/bin/direa-js
ENV DIREA_JS_ADAPTER=/opt/die-a-rea/adapters/js/direa-js

COPY eval/entrypoint.sh /usr/local/bin/run-testbowl
COPY eval/repos /testbowl-repos
RUN chmod +x /usr/local/bin/run-testbowl
WORKDIR /testbowl
ENTRYPOINT ["/usr/local/bin/run-testbowl"]
CMD ["all"]
