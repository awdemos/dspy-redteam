# syntax=docker/dockerfile:1.7

# This file uses multi-stage builds to build the application from source, including the front-end

# Tags passed to "go build"
ARG BUILD_TAGS=""
ARG CACHE_BUST="0"

# Stage 1: Build Frontend
FROM node:24-alpine AS frontend-builder
RUN corepack enable

WORKDIR /build

COPY package.json pnpm-workspace.yaml pnpm-lock.yaml ./
COPY frontend/package.json ./frontend/
RUN --mount=type=cache,target=/root/.local/share/pnpm/store \
  pnpm --filter pocket-id-frontend install --frozen-lockfile

COPY ./frontend ./frontend/

RUN --mount=type=cache,target=/build/frontend/node_modules/.vite \
  --mount=type=cache,target=/build/frontend/.svelte-kit \
  BUILD_OUTPUT_PATH=dist pnpm --filter pocket-id-frontend run build

# Stage 2: Build Backend
FROM golang:1.26.4-alpine AS backend-builder
ARG BUILD_TAGS
ARG CACHE_BUST
WORKDIR /build
COPY ./backend/go.mod ./backend/go.sum ./
RUN --mount=type=cache,target=/go/pkg/mod \
  go mod download

COPY ./backend ./
COPY --from=frontend-builder /build/frontend/dist ./frontend/dist
COPY .version .version

WORKDIR /build/cmd
# Bust the go-build cache when backend source or the cache-bust token changes.
# This ensures overlays like one_time_access_service.go are recompiled even though
# the upstream tag is the same.
RUN --mount=type=cache,target=/go/pkg/mod \
  VERSION=$(cat /build/.version) && \
  echo "cache-bust=${CACHE_BUST}" && \
  CGO_ENABLED=0 \
  GOOS=linux \
  go build \
    -tags "${BUILD_TAGS}" \
    -ldflags="-X github.com/pocket-id/pocket-id/backend/internal/common.Version=${VERSION} -buildid=${VERSION}-${CACHE_BUST}" \
    -trimpath \
    -o /build/pocket-id \
    .

# Stage 3: Production Image
FROM alpine:3.24.1
WORKDIR /app

RUN apk add --no-cache curl su-exec

COPY --from=backend-builder /build/pocket-id /app/pocket-id
COPY ./scripts/docker /app/docker

RUN chmod +x /app/pocket-id && \
  find /app/docker -name "*.sh" -exec chmod +x {} \;

EXPOSE 1411
ENV APP_ENV=production

HEALTHCHECK --interval=90s --timeout=5s --start-period=10s --retries=3 CMD [ "/app/pocket-id", "healthcheck" ]

ENTRYPOINT ["sh", "/app/docker/entrypoint.sh"]
CMD ["/app/pocket-id"]
