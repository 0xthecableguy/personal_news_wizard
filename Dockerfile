FROM rust:1.71 AS builder

WORKDIR /usr/src/news_wizard
COPY . .
RUN cargo build --release
FROM debian:bullseye-slim
RUN apt-get update && apt-get install -y libssl-dev pkg-config ca-certificates && rm -rf /var/lib/apt/lists/*
WORKDIR /usr/src/news_wizard

COPY --from=builder /usr/src/news_wizard/target/release/news_wizard .
COPY --from=builder /usr/src/news_wizard/.env .env

COPY --from=builder /usr/src/news_wizard/common_res /usr/src/news_wizard/common_res
COPY --from=builder /usr/src/news_wizard/localization /usr/src/news_wizard/localization

RUN mkdir -p /usr/src/news_wizard/tmp /usr/src/news_wizard/users_sessions

VOLUME ["/usr/src/news_wizard/common_res", "/usr/src/news_wizard/localization", "/usr/src/news_wizard/tmp", "/usr/src/news_wizard/users_sessions"]
ENV RUST_LOG=info
CMD ["./news_wizard"]