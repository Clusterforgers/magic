# The game server and the data tool in one image: `mtg-server` serves, `mtg-cli` migrates and loads card data.
FROM rust:1-bookworm AS build
# The toolchain file asks for rustfmt and clippy; the build needs neither.
ENV RUSTUP_TOOLCHAIN=stable
WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml clippy.toml ./
COPY .cargo .cargo
COPY .sqlx .sqlx
COPY crates crates
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/usr/local/cargo/git \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p mtg-server -p mtg-cli \
    && install -D target/release/mtg-server target/release/mtg-cli -t /out

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --uid 10001 --create-home --shell /usr/sbin/nologin magic
COPY --from=build /out/mtg-server /out/mtg-cli /usr/local/bin/
USER 10001
ENV MTG_BIND=0.0.0.0:8080
EXPOSE 8080
CMD ["mtg-server"]
