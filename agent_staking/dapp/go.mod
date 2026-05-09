module agent-staking

go 1.21

require (
	github.com/bytecodealliance/wasmtime-go v1.0.0
	github.com/rubixchain/rubix-wasm/go-wasm-bridge v0.0.0-20241021011146-a8b29487213e
)

require github.com/gorilla/websocket v1.5.3 // indirect

replace github.com/rubixchain/rubix-wasm/go-wasm-bridge => ../../../../Rubix/rubix-wasm/go-wasm-bridge
