package host

import (
	"fmt"

	"agent-staking/db"

	"github.com/bytecodealliance/wasmtime-go"
	wasmContext "github.com/rubixchain/rubix-wasm/go-wasm-bridge/context"
	wasmhost "github.com/rubixchain/rubix-wasm/go-wasm-bridge/host"
)

// AppendBatch implements the append_batch_to_storage host function.
// WASM passes (batch_hash_ptr, batch_hash_len, batch_json_ptr, batch_json_len) → i32.
type AppendBatch struct {
	allocFunc *wasmtime.Func
	memory    *wasmtime.Memory
}

func NewAppendBatch() *AppendBatch { return &AppendBatch{} }

func (h *AppendBatch) Name() string { return "append_batch_to_storage" }

func (h *AppendBatch) FuncType() *wasmtime.FuncType {
	return wasmtime.NewFuncType(
		[]*wasmtime.ValType{
			wasmtime.NewValType(wasmtime.KindI32), // batch_hash_ptr
			wasmtime.NewValType(wasmtime.KindI32), // batch_hash_len
			wasmtime.NewValType(wasmtime.KindI32), // batch_json_ptr
			wasmtime.NewValType(wasmtime.KindI32), // batch_json_len
		},
		[]*wasmtime.ValType{wasmtime.NewValType(wasmtime.KindI32)},
	)
}

func (h *AppendBatch) Initialize(allocFunc, _ *wasmtime.Func, memory *wasmtime.Memory, _ string, _ int, _ *wasmContext.WasmContext) {
	h.allocFunc = allocFunc
	h.memory = memory
}

func (h *AppendBatch) Callback() wasmhost.HostFunctionCallBack {
	return h.callback
}

func (h *AppendBatch) callback(caller *wasmtime.Caller, args []wasmtime.Val) ([]wasmtime.Val, *wasmtime.Trap) {
	if len(args) != 4 {
		return errI32(fmt.Sprintf("%v expects 4 args, got %d", h.Name(), len(args)))
	}

	mem := caller.GetExport("memory").Memory()
	if mem == nil {
		return errI32("memory export not found")
	}

	data := mem.UnsafeData(caller)
	memSize := mem.DataSize(caller)

	batchHash, trap := readString(data, memSize, args[0].I32(), args[1].I32(), h.Name()+" hash")
	if trap != nil {
		return []wasmtime.Val{wasmtime.ValI32(1)}, trap
	}
	batchJSON, trap := readString(data, memSize, args[2].I32(), args[3].I32(), h.Name()+" json")
	if trap != nil {
		return []wasmtime.Val{wasmtime.ValI32(1)}, trap
	}

	d := db.GetGlobal()
	if d == nil {
		return errI32("db not initialised")
	}
	if err := d.AppendBatch(batchHash, batchJSON); err != nil {
		return errI32(fmt.Sprintf("AppendBatch: %v", err))
	}

	return []wasmtime.Val{wasmtime.ValI32(0)}, nil
}
