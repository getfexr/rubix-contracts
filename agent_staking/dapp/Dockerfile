FROM golang:1.21-alpine AS builder

RUN apk add --no-cache gcc musl-dev

WORKDIR /build

# Copy go module files first for layer caching
COPY go.mod go.sum ./

# Copy vendor directory (run `go mod vendor` locally before building)
COPY vendor/ vendor/

# Copy source
COPY . .

RUN CGO_ENABLED=0 GOFLAGS=-mod=vendor go build -o dapp .

# ──────────────────────────────────────────────────────────────────────────────

FROM alpine:3.19

RUN apk add --no-cache ca-certificates wget

WORKDIR /app

COPY --from=builder /build/dapp /app/dapp

RUN mkdir -p /data /app

EXPOSE 6001

ENTRYPOINT ["/app/dapp"]
