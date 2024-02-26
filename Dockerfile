FROM rust:1.75-slim-bookworm as builder

RUN apt-get update && apt-get -y install protobuf-compiler
WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim as runner
RUN apt-get update
RUN apt-get install -y --fix-missing pcscd yubikey-manager python3 python3-pip gnupg libc6 sysvinit-utils pcsc-tools libccid libnss3-tools openssl
RUN apt-get clean

FROM runner
RUN service pcscd start
WORKDIR /app
COPY --from=builder /app/target/release/yubikey-provision /usr/local/bin
ENTRYPOINT ["/usr/local/bin/yubikey-provision"]
