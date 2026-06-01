package host

import (
	"fmt"

	"agent-staking/db"

	"github.com/bytecodealliance/wasmtime-go"
	wasmContext "github.com/rubixchain/rubix-wasm/go-wasm-bridge/context"
	wasmhost "github.com/rubixchain/rubix-wasm/go-wasm-bridge/host"
)

// GetActivityCount implements the get_activity_count_in_range host function.
// WASM passes (from_ts: i64, to_ts: i64) → i64 (count, or -1 on error).
type GetActivityCount struct{}

func NewGetActivityCount() *GetActivityCount { return &GetActivityCount{} }

func (h *GetActivityCount) Name() string { return "get_activity_count_in_range" }

func (h *GetActivityCount) FuncType() *wasmtime.FuncType {
	return wasmtime.NewFuncType(
		[]*wasmtime.ValType{
			wasmtime.NewValType(wasmtime.KindI64), // from_ts (unix ms)
			wasmtime.NewValType(wasmtime.KindI64), // to_ts (unix ms)
		},
		[]*wasmtime.ValType{wasmtime.NewValType(wasmtime.KindI64)}, // count
	)
}

func (h *GetActivityCount) Initialize(_ *wasmtime.Func, _ *wasmtime.Func, _ *wasmtime.Memory, _ string, _ int, _ *wasmContext.WasmContext) {
}

func (h *GetActivityCount) Callback() wasmhost.HostFunctionCallBack {
	return h.callback
}

func (h *GetActivityCount) callback(caller *wasmtime.Caller, args []wasmtime.Val) ([]wasmtime.Val, *wasmtime.Trap) {
	if len(args) != 2 {
		return []wasmtime.Val{wasmtime.ValI64(-1)},
			wasmtime.NewTrap(fmt.Sprintf("%v expects 2 args, got %d", h.Name(), len(args)))
	}

	fromTs := uint64(args[0].I64())
	toTs := uint64(args[1].I64())

	d := db.GetGlobal()
	if d == nil {
		return []wasmtime.Val{wasmtime.ValI64(-1)}, nil
	}

	count, err := d.GetActivityCountInRange(fromTs, toTs)
	if err != nil {
		return []wasmtime.Val{wasmtime.ValI64(-1)}, nil
	}

	return []wasmtime.Val{wasmtime.ValI64(count)}, nil
}
