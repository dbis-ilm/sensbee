# Based on https://dev.to/johndotowl/building-and-running-a-rust-application-with-docker-on-mac-apple-silicon-1p88
################################################################################
# Create a stage for building the application.

ARG RUST_VERSION=1.87.0

FROM rust:${RUST_VERSION}-slim-bookworm AS build

RUN apt-get update && apt-get upgrade -y && \
    apt-get install -y pkg-config openssl libssl-dev ca-certificates curl && \
    apt-get clean

RUN ln -sf /bin/bash /bin/sh

WORKDIR /app

# Build the application.
# Leverage a cache mount to /usr/local/cargo/registry/
# for downloaded dependencies and a cache mount to /app/target/ for 
# compiled dependencies which will speed up subsequent builds.
# Leverage a bind mount to the src directory to avoid having to copy the
# source code into the container. Once built, copy the executable to an
# output directory before the cache mounted /app/target is unmounted.
RUN --mount=type=bind,source=sensor_mgmt,target=sensor_mgmt \
    --mount=type=bind,source=sbc,target=sbc \    
    --mount=type=bind,source=server,target=server \
    --mount=type=bind,source=event_handling,target=event_handling \
    --mount=type=bind,source=openapi,target=openapi \
    --mount=type=bind,source=migrations,target=migrations \
    --mount=type=bind,source=Cargo.toml,target=Cargo.toml \
    --mount=type=bind,source=.sqlx,target=.sqlx \
    --mount=type=bind,source=.env,target=.env \
    --mount=type=cache,target=/app/target/ \
    --mount=type=cache,target=/usr/local/cargo/registry/ \
    /bin/sh -c "cargo build --release && \
    cp ./target/release/sb_* /bin/"

################################################################################
# Create a new stage for running the application that contains the minimal
# runtime dependencies for the application. This often uses a different base
# image from the build stage where the necessary files are copied from the build
# stage.
FROM debian:bookworm-slim AS base_runtime

RUN apt-get update && \
    apt-get install -y pkg-config openssl libssl3 libssl-dev ca-certificates curl

# Create a non-privileged user that the app will run under.
# See https://docs.docker.com/develop/develop-images/dockerfile_best-practices/#user
ARG UID=10001
RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    appuser
USER appuser

# Create directory for config files
USER root
RUN mkdir -p /etc/sensbee/ && chown -R appuser /etc/sensbee

################################################################################
# Create a new stage for running the application that contains the minimal
# runtime dependencies for the application. This often uses a different base
# image from the build stage where the necessary files are copied from the build
# stage.

FROM base_runtime AS server

ENV SB_CONTAINER="true"

# Copy the executables from the "build" stage.
COPY --from=build "/bin" "/bin"

# Start the server.
ENTRYPOINT ["/bin/sb_srv"]

# Expose the port that the application listens on.
EXPOSE 8080

################################################################################
# Create a new stage for running the application that contains the minimal
# runtime dependencies for the application. This often uses a different base
# image from the build stage where the necessary files are copied from the build
# stage.
FROM base_runtime AS event_handler

ENV SB_CONTAINER="true"

# Copy the executables from the "build" stage.
COPY --from=build "/bin/sb_event_handler" "/bin/sb_event_handler"

# Start the server.
ENTRYPOINT ["/bin/sb_event_handler"]

################################################################################
# OpenAPI documentation server

FROM base_runtime AS openapi

# Copy the executables from the "build" stage.
COPY --from=build "/bin/sb_openapi" "/bin/sb_openapi"

# Start the server.
ENTRYPOINT ["/bin/sb_openapi"]

################################################################################
# CI Testing container

FROM build AS ci_test

ENV SB_CONTAINER="true"

RUN cargo install cargo-tarpaulin

# Keep the container running
ENTRYPOINT ["tail", "-f", "/dev/null"]