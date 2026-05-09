package host

import (
	"fmt"

	"agent-staking/state"

	"github.com/bytecodealliance/wasmtime-go"
	wasmContext "github.com/rubixchain/rubix-wasm/go-wasm-bridge/context"
	wasmhost "github.com/rubixchain/rubix-wasm/go-wasm-bridge/host"
)

type SaveAgentState struct {
	allocFunc *wasmtime.Func
	memory    *wasmtime.Memory
}

func NewSaveAgentState() *SaveAgentState {
	return &SaveAgentState{}
}

func (h *SaveAgentState) Name() string {
	return "save_agent_state_to_storage"
}

func (h *SaveAgentState) FuncType() *wasmtime.FuncType {
	return wasmtime.NewFuncType(
		[]*wasmtime.ValType{
			wasmtime.NewValType(wasmtime.KindI32), // in_state_ptr
			wasmtime.NewValType(wasmtime.KindI32), // in_state_len
		},
		[]*wasmtime.ValType{wasmtime.NewValType(wasmtime.KindI32)},
	)
}

func (h *SaveAgentState) Initialize(allocFunc, _ *wasmtime.Func, memory *wasmtime.Memory, _ string, _ int, _ *wasmContext.WasmContext) {
	h.allocFunc = allocFunc
	h.memory = memory
}

func (h *SaveAgentState) Callback() wasmhost.HostFunctionCallBack {
	return h.callback
}

func (h *SaveAgentState) callback(caller *wasmtime.Caller, args []wasmtime.Val) ([]wasmtime.Val, *wasmtime.Trap) {
	if len(args) != 2 {
		msg := fmt.Sprintf("%v expects 2 arguments, got %d", h.Name(), len(args))
		return []wasmtime.Val{wasmtime.ValI32(1)}, wasmtime.NewTrap(msg)
	}

	inPtr := args[0].I32()
	inLen := args[1].I32()

	mem := caller.GetExport("memory").Memory()
	if mem == nil {
		return []wasmtime.Val{wasmtime.ValI32(1)}, wasmtime.NewTrap("memory export not found")
	}
	h.memory = mem

	data := mem.UnsafeData(caller)
	start := int(inPtr)
	end := start + int(inLen)
	if start < 0 || end > len(data) {
		return []wasmtime.Val{wasmtime.ValI32(1)}, wasmtime.NewTrap("input state exceeds memory bounds")
	}

	stateJSON := string(data[start:end])
	if err := state.SaveStateFromJSON(stateJSON); err != nil {
		return []wasmtime.Val{wasmtime.ValI32(1)}, wasmtime.NewTrap(fmt.Sprintf("failed to save state: %v", err))
	}

	return []wasmtime.Val{wasmtime.ValI32(0)}, nil
}
