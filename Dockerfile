FROM rust:latest
WORKDIR /usr/src/s3mon
COPY . .
RUN cargo build --release

FROM debian:latest
RUN apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y openssl ca-certificates
COPY --from=0 /usr/src/s3mon/target/release/s3mon /
CMD ["./s3mon", "-h"]
