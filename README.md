# Booking System - Backend Service

Scheduling and booking engine written in Rust. This project is a Proof of Concept for a predecessor for the OrSee 
framework

## Prerequisites

- **Rust**
- **Docker**: For PostgreSQL and Mail Proxy services
- **OpenSSL**: Required for JWT key generation

## Quick Start

### 1. Environment Setup

Copy the example environment file and configure the database/JWT keys.

```bash
cp .env.example .env
```

*Note: Ensure `JWT_PRIVATE_KEY` and `JWT_PUBLIC_KEY` are valid Ed25519 PEM keys.*

```bash
# Generate Private Key
openssl genpkey -algorithm ED25519 -out private.pem

# Extract Public Key
openssl pkey -in private.pem -pubout -out public.pem
```

### 2. Start Infrastructure

Start the PostgreSQL database and Mail Proxy using Docker Compose.

```bash
docker-compose up -d
```

### 3. Run Migrations

Initialize the database schema (requires `sqlx-cli`).

```bash
cargo install sqlx-cli
sqlx migrate run --source migrations/postgres
```

### 4. Run Application

Start the API server on `0.0.0.0:8000`.

```bash
cargo run --release
```

## Testing

Run the integration tests.

```bash
cargo test
```

## License

Copyright (c) 2025 Tobias Friedrich. Licensed under the GNU Affero General Public License v3.0 (AGPLv3).