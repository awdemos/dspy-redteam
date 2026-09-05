# syntax=docker/dockerfile:1

# Build stage
FROM cgr.dev/chainguard/rust:latest AS builder

USER root
WORKDIR /app

# Copy manifests and enough source structure for cargo to resolve targets
COPY Cargo.toml Cargo.lock ./
COPY src ./src

RUN cargo fetch

# Copy runtime assets
COPY templates ./templates
COPY static ./static
COPY migrations ./migrations

# Build release binary
RUN cargo build --release

# Runtime stage
FROM cgr.dev/chainguard/wolfi-base

RUN apk add --no-cache ca-certificates curl

WORKDIR /app

COPY --from=builder /app/target/release/redcell /usr/local/bin/redcell
COPY --from=builder /app/migrations /app/migrations
COPY --from=builder /app/static /app/static
COPY --from=builder /app/templates /app/templates

RUN mkdir -p /app/data && chown -R nonroot:nonroot /app

EXPOSE 3000

ENV REDTEAM__DATABASE__URL=sqlite:///app/data/redcell.db
ENV REDTEAM__SERVER__HOST=0.0.0.0
ENV REDTEAM__SERVER__PORT=3000

USER nonroot

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -sf http://localhost:3000/health || exit 1

CMD ["redcell"]
