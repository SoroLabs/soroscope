# Build stage
FROM rust:1.80-slim-bookworm AS builder
WORKDIR /app
COPY . .
RUN cargo build --release -p soroscope-core

# Final stage
FROM debian:bookworm-slim
# Install required libraries
RUN apt-get update && apt-get install -y libssl-dev ca-certificates curl && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/soroscope-core .
EXPOSE 8080
CMD ["./soroscope-core"]
