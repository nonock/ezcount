# syntax=docker/dockerfile:1
# ezcount sync relay. Build from this directory:
#   docker build -t ezcount-relay sync-server
#   docker run -d -p 8787:8787 -v ezcount-relay:/data ezcount-relay

FROM rust:1-slim-bookworm AS build
WORKDIR /src
COPY . .
# Cache mounts keep the registry and build artifacts between builds, so only changed crates
# recompile. The binary is copied out because the target cache isn't part of the image.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked \
    && cp target/release/ezcount-sync-server /ezcount-sync-server
# Distroless has no shell, so prepare the data directory here for the non-root user.
RUN mkdir /data

# Fly.io (fly.toml's build target): its volumes are mounted owned by root, and every app
# already runs in its own VM, so this variant runs as root.
FROM gcr.io/distroless/cc-debian12 AS fly
COPY --from=build /ezcount-sync-server /usr/local/bin/ezcount-sync-server
ENV EZCOUNT_SYNC_ADDR=0.0.0.0:8787 \
    EZCOUNT_SYNC_DB=/data/ezcount-sync.sqlite3
EXPOSE 8787
ENTRYPOINT ["/usr/local/bin/ezcount-sync-server"]

# Default image: glibc + libgcc only, runs as uid 65532.
FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=build /ezcount-sync-server /usr/local/bin/ezcount-sync-server
COPY --from=build --chown=65532:65532 /data /data
ENV EZCOUNT_SYNC_ADDR=0.0.0.0:8787 \
    EZCOUNT_SYNC_DB=/data/ezcount-sync.sqlite3
VOLUME /data
EXPOSE 8787
# The relay shuts down cleanly on Ctrl-C; `docker stop` sends SIGTERM by default.
STOPSIGNAL SIGINT
ENTRYPOINT ["/usr/local/bin/ezcount-sync-server"]
