package host

import (
	"fmt"

	"agent-staking/db"

	"github.com/bytecodealliance/wasmtime-go"
	wasmContext "github.com/rubixchain/rubix-wasm/go-wasm-bridge/context"
	wasmhost "github.com/rubixchain/rubix-wasm/go-wasm-bridge/host"
)

// AppendActivity implements the append_activity_to_storage host function.
// WASM passes (activity_json_ptr, activity_json_len) → i32.
type AppendActivity struct {
	allocFunc *wasmtime.Func
	memory    *wasmtime.Memory
}

func NewAppendActivity() *AppendActivity { return &AppendActivity{} }

func (h *AppendActivity) Name() string { return "append_activity_to_storage" }

func (h *AppendActivity) FuncType() *wasmtime.FuncType {
	return wasmtime.NewFuncType(
		[]*wasmtime.ValType{
			wasmtime.NewValType(wasmtime.KindI32), // activity_json_ptr
			wasmtime.NewValType(wasmtime.KindI32), // activity_json_len
		},
		[]*wasmtime.ValType{wasmtime.NewValType(wasmtime.KindI32)},
	)
}

func (h *AppendActivity) Initialize(allocFunc, _ *wasmtime.Func, memory *wasmtime.Memory, _ string, _ int, _ *wasmContext.WasmContext) {
	h.allocFunc = allocFunc
	h.memory = memory
}

func (h *AppendActivity) Callback() wasmhost.HostFunctionCallBack {
	return h.callback
}

func (h *AppendActivity) callback(caller *wasmtime.Caller, args []wasmtime.Val) ([]wasmtime.Val, *wasmtime.Trap) {
	if len(args) != 2 {
		return errI32(fmt.Sprintf("%v expects 2 args, got %d", h.Name(), len(args)))
	}

	mem := caller.GetExport("memory").Memory()
	if mem == nil {
		return errI32("memory export not found")
	}

	data := mem.UnsafeData(caller)
	memSize := mem.DataSize(caller)

	activityJSON, trap := readString(data, memSize, args[0].I32(), args[1].I32(), h.Name())
	if trap != nil {
		return []wasmtime.Val{wasmtime.ValI32(1)}, trap
	}

	d := db.GetGlobal()
	if d == nil {
		return errI32("db not initialised")
	}
	if err := d.AppendActivity(activityJSON); err != nil {
		return errI32(fmt.Sprintf("AppendActivity: %v", err))
	}

	return []wasmtime.Val{wasmtime.ValI32(0)}, nil
}
