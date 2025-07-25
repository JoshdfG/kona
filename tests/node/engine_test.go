package node

import (
	"sync"
	"testing"

	"github.com/ethereum-optimism/optimism/op-devstack/devtest"
	"github.com/ethereum-optimism/optimism/op-devstack/dsl"
	"github.com/ethereum-optimism/optimism/op-supervisor/supervisor/types"
	"github.com/stretchr/testify/require"
)

func TestEngine(gt *testing.T) {
	t := devtest.ParallelT(gt)

	out := NewMixedOpKona(t)

	// Get the nodes from the network.
	nodes := out.L2CLNodes()

	wg := sync.WaitGroup{}
	for _, node := range nodes {
		clName := node.Escape().ID().Key()
		clRPC, err := GetNodeRPCEndpoint(t.Ctx(), &node)

		// See if the node supports the dev RPC.
		if !supportsDevRPC(t, clName, clRPC) {
			t.Log("node does not support dev RPC, skipping engine test for", node.Escape().ID().Key())
			continue
		}

		t.Log("node supports dev RPC, running engine test for", node.Escape().ID().Key())

		wg.Add(1)
		go func(node dsl.L2CLNode) {
			defer wg.Done()

			require.NoError(t, err, "failed to get RPC endpoint for node %s", clName)

			// Wait group to wait for 50 unsafe blocks to be produced.
			outerWg := sync.WaitGroup{}

			outerWg.Add(1)

			queue := make(chan []uint64)

			// Spawn a task that gets the engine queue length with a ws connection.
			go func() {
				wsRPC := websocketRPC(clRPC)

				// Create a channel that completes when outerWg.Wait() completes
				done := make(chan struct{})
				go func() {
					outerWg.Wait()
					done <- struct{}{}
				}()

				queue <- GetDevWS(t, wsRPC, "engine_queue_size", done)
			}()

			// Wait for 40 unsafe blocks to be produced.
			node.Advanced(types.LocalUnsafe, 40, 100)

			outerWg.Done()

			q := <-queue
			for _, q := range q {
				require.LessOrEqual(t, q, uint64(1), "engine queue length should be 1 or less")
			}
		}(node)
	}

	wg.Wait()

}
