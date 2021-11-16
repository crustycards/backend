FROM rust:1.56.1 as build
WORKDIR /app
COPY / /app
RUN \
    # Install git submodules.
    git submodule update --init --recursive &&\
    # Install cargo fmt (needed for prost code generation).
    rustup component add rustfmt &&\
    cargo build --release --package game_service &&\
    mkdir -p /build-out &&\
    cp target/release/game_service /build-out/

FROM debian:10-slim
COPY --from=build /build-out/game_service /
EXPOSE 50052
CMD /game_service
