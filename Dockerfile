FROM rust:1.92.0-slim-bookworm AS builder

WORKDIR /workspace
COPY . .
RUN cargo build -p decision-gate-cli --release --locked

FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

RUN groupadd --gid 10001 decision-gate \
    && useradd --uid 10001 --gid 10001 --home-dir /nonexistent --shell /usr/sbin/nologin decision-gate \
    && mkdir -p /etc/decision-gate /var/lib/decision-gate \
    && chown -R decision-gate:decision-gate /etc/decision-gate /var/lib/decision-gate

COPY --from=builder /workspace/target/release/decision-gate /usr/local/bin/decision-gate

USER decision-gate
WORKDIR /

EXPOSE 8080
ENTRYPOINT ["decision-gate"]
CMD ["serve", "--config", "/etc/decision-gate/decision-gate.toml", "--allow-non-loopback"]
