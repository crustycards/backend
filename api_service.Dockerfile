FROM rust:1.51.0 as build
WORKDIR /app
COPY / /app
RUN \
    # Install protocol buffers.
    PROTOC_ZIP=protoc-3.11.4-linux-x86_64.zip &&\
    curl -OL https://github.com/protocolbuffers/protobuf/releases/download/v3.11.4/$PROTOC_ZIP &&\
    unzip -o $PROTOC_ZIP -d /usr/local bin/protoc &&\
    unzip -o $PROTOC_ZIP -d /usr/local 'include/*' &&\
    rm -f $PROTOC_ZIP &&\
    # Install git submodules.
    git submodule update --init --recursive &&\
    # Install cargo fmt (needed for prost code generation).
    rustup component add rustfmt &&\
    cargo build --release --package api_service &&\
    mkdir -p /build-out &&\
    cp target/release/api_service /build-out/

FROM debian:10-slim
COPY --from=build /build-out/api_service /
EXPOSE 50052
CMD /api_service
