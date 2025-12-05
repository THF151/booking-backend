FROM rust:1.91-alpine AS builder

RUN apk add --no-cache \
    musl-dev \
    openssl-dev \
    pkgconfig \
    make \
    openssl-libs-static

WORKDIR /app

RUN cargo new --bin booking-backend
WORKDIR /app/booking-backend

COPY ./Cargo.toml ./Cargo.lock ./

RUN cargo build --release
RUN rm src/*.rs

COPY ./src ./src
COPY ./migrations ./migrations

RUN touch src/main.rs

ENV SQLX_OFFLINE=true
ENV OPENSSL_STATIC=1
RUN cargo build --release

FROM alpine:3.22

RUN apk add --no-cache libgcc ca-certificates

RUN addgroup -S appgroup && adduser -S appuser -G appgroup

WORKDIR /app

COPY --from=builder /app/booking-backend/target/release/booking-backend .

RUN chown -R appuser:appgroup /app

USER appuser

ENV PORT=8000
ENV RUST_LOG=info
ENV DATABASE_URL=""

EXPOSE 8000

CMD ["./booking-backend"]