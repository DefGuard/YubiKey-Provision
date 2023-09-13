FROM rust:slim as builder

RUN apt-get update && apt-get -y install protobuf-compiler gcc-multilib musl-tools
WORKDIR /app
COPY . .
RUN rustup target add x86_64-unknown-linux-musl
RUN cargo build --release --target x86_64-unknown-linux-musl

FROM debian:bullseye-slim as runner
RUN apt-get update
RUN apt-get install -y pcscd yubikey-manager python3 python3-pip gnupg libc6 sysvinit-utils pcsc-tools libccid libnss3-tools openssl
RUN apt-get clean
RUN pip install yubikey-manager

FROM runner
RUN service pcscd start
WORKDIR /app
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/yubikey-provision /usr/local/bin
ENTRYPOINT ["/usr/local/bin/yubikey-provider"]
