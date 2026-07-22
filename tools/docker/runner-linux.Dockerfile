FROM node:24-bullseye-slim AS node

FROM ubuntu:22.04

ENV DEBIAN_FRONTEND=noninteractive
ENV CARGO_HOME=/root/.cargo
ENV PATH=/root/.cargo/bin:/usr/local/bin:$PATH
ENV APPIMAGE_EXTRACT_AND_RUN=1

RUN apt-get update \
    && apt-get install --yes --no-install-recommends \
        build-essential \
        ca-certificates \
        curl \
        file \
        libasound2-dev \
        libayatana-appindicator3-dev \
        libfuse2 \
        libudev-dev \
        libwebkit2gtk-4.1-dev \
        librsvg2-dev \
        libxdo-dev \
        patchelf \
        pkg-config \
        rpm \
        xdg-utils \
    && rm -rf /var/lib/apt/lists/*

COPY --from=node /usr/local/ /usr/local/

RUN npm install --global pnpm@11.10.0 \
    && curl --proto '=https' --tlsv1.2 --fail --silent --show-error https://sh.rustup.rs \
        | sh -s -- --default-toolchain 1.95.0 --profile minimal --no-modify-path -y \
    && rustc --version \
    && cargo --version \
    && node --version \
    && pnpm --version
