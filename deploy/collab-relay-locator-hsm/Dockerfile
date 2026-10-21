# syntax=docker/dockerfile:1.7
ARG DOCKER_REGISTRY=docker.io

FROM ${DOCKER_REGISTRY}/library/rust:1.94-bookworm@sha256:6ae102bdbf528294bc79ad6e1fae682f6f7c2a6e6621506ba959f9685b308a55 AS build
WORKDIR /src
COPY . .
RUN cargo build --locked --release -p op-collab-relay-locator-hsm

FROM build AS test
RUN apt-get update && \
    apt-get install -y --no-install-recommends softhsm2 && \
    rm -rf /var/lib/apt/lists/*
ENV OPENPENCIL_SOFTHSM2_MODULE=/usr/lib/softhsm/libsofthsm2.so
RUN cargo test --locked -p op-collab-relay-locator-hsm --test softhsm -- --nocapture

FROM ${DOCKER_REGISTRY}/library/debian:bookworm-20260713-slim@sha256:7b140f374b289a7c2befc338f42ebe6441b7ea838a042bbd5acbfca6ec875818

ARG LOCATOR_UID=65532
ARG SIGNER_UID=65533
ARG SHARED_GID=65532

RUN apt-get update && \
    apt-get install -y --no-install-recommends softhsm2 && \
    rm -rf /var/lib/apt/lists/* && \
    groupadd --gid "${SHARED_GID}" openpencil-locator && \
    useradd --uid "${SIGNER_UID}" --gid "${SHARED_GID}" --no-create-home \
      --home-dir /nonexistent --shell /usr/sbin/nologin openpencil-locator-hsm && \
    test "${LOCATOR_UID}" != "${SIGNER_UID}" && \
    install -d -o "${SIGNER_UID}" -g "${SHARED_GID}" -m 0700 /var/lib/openpencil-softhsm/tokens

COPY --from=build /src/target/release/op-collab-relay-locator-hsm /usr/local/bin/op-collab-relay-locator-hsm
COPY --chmod=0444 deploy/collab-relay-locator-hsm/softhsm2.conf /etc/openpencil-softhsm2.conf

ENV SOFTHSM2_CONF=/etc/openpencil-softhsm2.conf
USER ${SIGNER_UID}:${SHARED_GID}

HEALTHCHECK --interval=30s --timeout=10s --start-period=10s --retries=3 \
  CMD ["/usr/local/bin/op-collab-relay-locator-hsm", "check", "--config", "/run/openpencil-config/locator-hsm.json"]

ENTRYPOINT ["/usr/local/bin/op-collab-relay-locator-hsm"]
CMD ["serve", "--config", "/run/openpencil-config/locator-hsm.json"]
