# Builds the worker daemon. It lives at the api/ root so the build context includes
# Cargo.toml and the modules it shares with the API.
# Matches rust-toolchain.toml, so rustup uses the image's toolchain instead of installing one.
FROM rust:1.95.0-bookworm AS build
WORKDIR /src

COPY . .
RUN cargo build --release --locked --bin worker

# distroless/cc has the glibc the binary links against, and the CA certificates the worker
# needs for HTTPS checks. It matches the build stage's Debian release.
FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=build /src/target/release/worker /worker
ENTRYPOINT ["/worker"]
