FROM rustlang/rust:nightly-alpine AS build
RUN apk add --no-cache musl-dev pkgconfig openssl-dev openssl-libs-static
WORKDIR /app
COPY Cargo.toml ./
COPY Cargo.lock ./
COPY src/ ./src/
RUN cargo build --release 2>&1

FROM alpine:3.19
RUN apk add --no-cache ca-certificates
COPY --from=build /app/target/release/agent-shield /usr/local/bin/agent-shield
COPY --from=build /app/target/release/ash-dashboard /usr/local/bin/ash-dashboard
COPY --from=build /app/target/release/ash-orchestrator /usr/local/bin/ash-orchestrator
EXPOSE 8888 9999
CMD ["agent-shield"]
