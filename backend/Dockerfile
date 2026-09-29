FROM golang:1.26-alpine AS builder
RUN apk add --no-cache git gcc musl-dev

WORKDIR /app
COPY ./backend/go.mod ./backend/go.sum* ./
RUN go mod download

COPY ./backend .
RUN CGO_ENABLED=1 GOOS=linux go build -o main ./cmd/api

# runtime
FROM alpine:latest
RUN apk add --no-cache git openssh-client

WORKDIR /app
COPY --from=builder /app/main .
RUN mkdir -p /app/data/cities

EXPOSE 8080
CMD ["./main"]
