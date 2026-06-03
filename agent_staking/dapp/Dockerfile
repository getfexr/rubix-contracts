FROM golang:1.21-alpine AS builder

RUN apk add --no-cache gcc musl-dev git

WORKDIR /build

# Copy source
COPY . .

# The go.mod replace directive points to a local path that doesn't exist inside
# Docker. Clone rubix-wasm at the exact pseudoversion commit and redirect the
# replace to the cloned path so the build resolves identically to the dev machine.
RUN git clone --no-checkout https://github.com/rubixchain/rubix-wasm.git /rubix-wasm && \
    cd /rubix-wasm && git checkout a8b29487213e

RUN go mod edit -replace=github.com/rubixchain/rubix-wasm/go-wasm-bridge=/rubix-wasm/go-wasm-bridge

RUN go mod download

RUN CGO_ENABLED=0 go build -o dapp .

# ──────────────────────────────────────────────────────────────────────────────

FROM alpine:3.19

RUN apk add --no-cache ca-certificates wget

WORKDIR /app

COPY --from=builder /build/dapp /app/dapp

RUN mkdir -p /data /app

EXPOSE 6001

ENTRYPOINT ["/app/dapp"]
