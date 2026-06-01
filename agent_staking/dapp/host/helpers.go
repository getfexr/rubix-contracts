package host

import (
	"fmt"
	"log"

	"github.com/bytecodealliance/wasmtime-go"
)

// readString reads a UTF-8 string from WASM linear memory.
// Returns a trap only on memory bounds violations (unrecoverable).
func readString(data []byte, memSize uintptr, ptr, length int32, label string) (string, *wasmtime.Trap) {
	if ptr < 0 || length < 0 {
		return "", wasmtime.NewTrap(fmt.Sprintf("%s: negative ptr or length", label))
	}
	start := int(ptr)
	end := start + int(length)
	if end > len(data) || uintptr(end) > memSize {
		return "", wasmtime.NewTrap(fmt.Sprintf("%s: bounds exceeded (ptr=%d len=%d memSize=%d)",
			label, ptr, length, memSize))
	}
	return string(data[start:end]), nil
}

// errI32 logs msg and returns i32(1) without a trap so that Rust can read the
// error code and propagate it as a WasmError instead of aborting execution.
func errI32(msg string) ([]wasmtime.Val, *wasmtime.Trap) {
	log.Printf("[host] error: %s", msg)
	return []wasmtime.Val{wasmtime.ValI32(1)}, nil
}
