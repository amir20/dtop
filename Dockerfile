# Multi-stage build for docker-monitor (amd64/arm64)
FROM --platform=$BUILDPLATFORM rust:1.90-slim AS builder

ARG TARGETPLATFORM
ARG BUILDPLATFORM

# Install base dependencies
RUN apt-get update && apt-get install -y pkg-config libssl-dev musl-tools && rm -rf /var/lib/apt/lists/*

# Install cross-compilation toolchain if needed
RUN if [ "$BUILDPLATFORM" != "$TARGETPLATFORM" ]; then \
    apt-get update && apt-get install -y \
    $([ "$TARGETPLATFORM" = "linux/amd64" ] && echo "gcc-x86-64-linux-gnu" || echo "gcc-aarch64-linux-gnu") \
    && rm -rf /var/lib/apt/lists/*; \
    fi

# Add Rust target
RUN RUST_TARGET=$([ "$TARGETPLATFORM" = "linux/amd64" ] && echo "x86_64-unknown-linux-musl" || echo "aarch64-unknown-linux-musl") && \
    rustup target add "$RUST_TARGET"

WORKDIR /usr/src/docker-monitor
COPY Cargo.toml Cargo.lock ./
COPY src ./src

# Build and strip binary
RUN set -ex; \
    RUST_TARGET=$([ "$TARGETPLATFORM" = "linux/amd64" ] && echo "x86_64-unknown-linux-musl" || echo "aarch64-unknown-linux-musl"); \
    if [ "$TARGETPLATFORM" = "linux/amd64" ]; then \
    [ "$BUILDPLATFORM" != "$TARGETPLATFORM" ] && export CC=x86_64-linux-gnu-gcc CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER=x86_64-linux-gnu-gcc && STRIP="x86_64-linux-gnu-strip" || STRIP="strip"; \
    else \
    [ "$BUILDPLATFORM" != "$TARGETPLATFORM" ] && export CC=aarch64-linux-gnu-gcc CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER=aarch64-linux-gnu-gcc && STRIP="aarch64-linux-gnu-strip" || STRIP="strip"; \
    fi; \
    cargo build --release --target "$RUST_TARGET"; \
    cp "target/$RUST_TARGET/release/docker-monitor" /usr/local/bin/docker-monitor; \
    "$STRIP" /usr/local/bin/docker-monitor

FROM scratch
COPY --from=builder /usr/local/bin/docker-monitor /docker-monitor
COPY --from=builder /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/
ENTRYPOINT ["/docker-monitor"]
CMD ["--host", "local"]
