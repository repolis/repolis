# syntax=docker/dockerfile:1
FROM golang:1.26.11-alpine3.24 AS builder
# go-tree-sitter is cgo.
RUN apk add --no-cache gcc musl-dev
WORKDIR /src
COPY backend/ ./
RUN --mount=type=cache,target=/go/pkg/mod \
    --mount=type=cache,target=/root/.cache/go-build \
    CGO_ENABLED=1 go build -o /out/repolis ./cmd/api

FROM alpine:3.24
RUN apk add --no-cache git ca-certificates
WORKDIR /app
COPY --from=builder /out/repolis ./repolis
EXPOSE 8081
CMD ["./repolis"]
