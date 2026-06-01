module agent-staking

go 1.21

require (
	github.com/bytecodealliance/wasmtime-go v1.0.0
	github.com/rubixchain/rubix-wasm/go-wasm-bridge v0.0.0-20241021011146-a8b29487213e
	modernc.org/sqlite v1.29.9
)

require (
	github.com/gorilla/websocket v1.5.3 // indirect
	modernc.org/libc v1.50.9 // indirect
	modernc.org/mathutil v1.6.0 // indirect
	modernc.org/memory v1.8.0 // indirect
)

replace github.com/rubixchain/rubix-wasm/go-wasm-bridge => ../../../../Rubix/rubix-wasm/go-wasm-bridge
