package host

import (
	"encoding/binary"
	"fmt"

	"agent-staking/state"

	"github.com/bytecodealliance/wasmtime-go"
	wasmContext "github.com/rubixchain/rubix-wasm/go-wasm-bridge/context"
	wasmhost "github.com/rubixchain/rubix-wasm/go-wasm-bridge/host"
)

type GetAgentState struct {
	allocFunc *wasmtime.Func
	memory    *wasmtime.Memory
}

func NewGetAgentState() *GetAgentState {
	return &GetAgentState{}
}

func (h *GetAgentState) Name() string {
	return "get_agent_state_from_storage"
}

func (h *GetAgentState) FuncType() *wasmtime.FuncType {
	return wasmtime.NewFuncType(
		[]*wasmtime.ValType{
			wasmtime.NewValType(wasmtime.KindI32), // out_state_ptr
			wasmtime.NewValType(wasmtime.KindI32), // out_state_len
		},
		[]*wasmtime.ValType{wasmtime.NewValType(wasmtime.KindI32)},
	)
}

func (h *GetAgentState) Initialize(allocFunc, _ *wasmtime.Func, memory *wasmtime.Memory, _ string, _ int, _ *wasmContext.WasmContext) {
	h.allocFunc = allocFunc
	h.memory = memory
}

func (h *GetAgentState) Callback() wasmhost.HostFunctionCallBack {
	return h.callback
}

func (h *GetAgentState) callback(caller *wasmtime.Caller, args []wasmtime.Val) ([]wasmtime.Val, *wasmtime.Trap) {
	if len(args) != 2 {
		msg := fmt.Sprintf("%v expects 2 arguments, got %d", h.Name(), len(args))
		return []wasmtime.Val{wasmtime.ValI32(1)}, wasmtime.NewTrap(msg)
	}

	outStatePtr := args[0].I32()
	outStateLen := args[1].I32()

	mem := caller.GetExport("memory").Memory()
	if mem == nil {
		return []wasmtime.Val{wasmtime.ValI32(1)}, wasmtime.NewTrap("memory export not found")
	}
	h.memory = mem

	stateJSON, err := state.GetStateJSON()
	if err != nil {
		return []wasmtime.Val{wasmtime.ValI32(1)}, wasmtime.NewTrap(fmt.Sprintf("failed to load state: %v", err))
	}

	respBytes := []byte(stateJSON)
	respLen := int32(len(respBytes))

	result, err := h.allocFunc.Call(caller, respLen)
	if err != nil {
		return []wasmtime.Val{wasmtime.ValI32(1)}, wasmtime.NewTrap(fmt.Sprintf("alloc failed: %v", err))
	}
	respPtr, ok := result.(int32)
	if !ok {
		return []wasmtime.Val{wasmtime.ValI32(1)}, wasmtime.NewTrap("alloc did not return i32")
	}

	data := mem.UnsafeData(caller)
	memSize := mem.DataSize(caller)
	if uint32(respPtr)+uint32(respLen) > uint32(memSize) {
		return []wasmtime.Val{wasmtime.ValI32(1)}, wasmtime.NewTrap("response exceeds memory bounds")
	}

	copy(data[respPtr:], respBytes)
	binary.LittleEndian.PutUint32(data[outStatePtr:], uint32(respPtr))
	binary.LittleEndian.PutUint32(data[outStateLen:], uint32(respLen))

	return []wasmtime.Val{wasmtime.ValI32(0)}, nil
}
