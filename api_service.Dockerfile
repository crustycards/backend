FROM rust:1.60.0 as build
WORKDIR /app
COPY / /app
RUN \
    # Install git submodules.
    git submodule update --init --recursive &&\
    # Install cargo fmt (needed for prost code generation).
    rustup component add rustfmt &&\
    cargo build --release --package api_service &&\
    mkdir -p /build-out &&\
    cp target/release/api_service /build-out/

FROM debian:11-slim
COPY --from=build /build-out/api_service /
EXPOSE 50052
CMD /api_service
