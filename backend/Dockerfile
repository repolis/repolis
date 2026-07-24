FROM golang:1.26-alpine AS builder
RUN apk add --no-cache git gcc musl-dev

WORKDIR /app
COPY ./backend/go.mod ./backend/go.sum* ./
RUN go mod download

COPY ./backend .
RUN CGO_ENABLED=0 GOOS=linux go build -o main ./cmd/api/main.go

# runtime
FROM alpine:latest
RUN apk add --no-cache git openssh-client

WORKDIR /root/
COPY --from=builder /app/main .

EXPOSE 8080
CMD ["./main"]
