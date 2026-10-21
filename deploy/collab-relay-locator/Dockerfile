# syntax=docker/dockerfile:1.7
# Multi-arch manifest digests verified from the upstream registries on
# 2026-07-29. Review and update both digests deliberately with each base-image
# refresh; tags remain only as human-readable provenance.
FROM rust:1.94-bookworm@sha256:6ae102bdbf528294bc79ad6e1fae682f6f7c2a6e6621506ba959f9685b308a55 AS build
WORKDIR /src
COPY . .
RUN cargo build --locked --release -p op-collab-relay-locator-server

FROM gcr.io/distroless/cc-debian12:nonroot@sha256:fccdbb0a547c14e23fcf4ce8ad62ca5d43b4faae8d22cd292f490fef9946c96e
COPY --from=build /src/target/release/op-collab-relay-locator-server /usr/local/bin/op-collab-relay-locator-server
EXPOSE 8092
ENTRYPOINT ["/usr/local/bin/op-collab-relay-locator-server", "--production"]
