FROM ubuntu:22.04 as builder

LABEL maintainer="Nikola Whallon <nikola@deepgram.com>"

RUN DEBIAN_FRONTEND=noninteractive apt-get update && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
        ca-certificates \
        clang \
        curl \
        libpq-dev \
        libssl-dev \
        pkg-config

COPY rust-toolchain /rust-toolchain
RUN curl https://sh.rustup.rs -sSf | sh -s -- -y --default-toolchain $(cat /rust-toolchain) && \
    . $HOME/.cargo/env

COPY . /deepgram-twilio-streaming-rust

RUN . $HOME/.cargo/env && \
    cargo install --path /deepgram-twilio-streaming-rust --root /

FROM ubuntu:22.04

LABEL maintainer="Nikola Whallon <nikola@deepgram.com>"

RUN DEBIAN_FRONTEND=noninteractive apt-get update && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y --no-install-recommends \
        ca-certificates \
        libpq5 \
        libssl3 && \
    DEBIAN_FRONTEND=noninteractive apt-get clean

COPY --from=builder /bin/deepgram-twilio-streaming-rust /bin/deepgram-twilio-streaming-rust

ENTRYPOINT ["/bin/deepgram-twilio-streaming-rust"]
CMD [""]
