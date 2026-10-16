# Oche agent kernel

This directory is the incremental boundary between a TypeScript agent host and
Oche. `pi_core.oche` owns scheduling state. `pi-core-driver.ts` loads its ESM
output and gives TypeScript a typed state-machine API.
`scheduler_policy.oche` contains the pure message, queue, tool-batch, and tool
declaration decisions. `pi_core.oche` imports it through the normal aliased Oche
module system while retaining the stable root kernel ABI consumed by TypeScript.

`jit-core-driver.ts` connects TypeScript to `oche serve`, which compiles the
same source to Kernel and keeps one Dart VM JIT process alive behind a
newline-delimited JSON protocol. Calls are asynchronous at this transport
boundary; checkpoints remain ordinary versioned JSON values.
pi-agent-core awaits asynchronous state-machine transitions, so this driver can
be passed directly through `AgentOptions.stateMachine`. The cross-repository
acceptance test drives the high-level `Agent` this way while provider streaming
and message ownership remain in TypeScript.
`OcheJitRuntime` owns the process and creates multiple state-machine adapters
that share it. Each adapter keeps its encoded state separately, allowing many
actors to use one VM without sharing scheduler state.
The runtime exposes a cumulative `callCount` so acceptance tests can enforce
the scheduler RPC budget independently of model and tool I/O.
The adapter implements pi-agent-core's scheduler checkpoint hooks, so a
high-level `Agent` can persist its encoded Oche state and lazily restore it into
another adapter attached to the same runtime.
`observe_message_batch` reduces transcript replay during start and restore to
one JIT RPC. Oche records the batch size and final message summary because the
scheduler does not retain a second transcript copy.
`begin_with_messages` also folds `start` or restored `enter` into that replay,
so opening an actor turn requires one scheduler RPC.
`restore_begin_with_messages` validates a prepared checkpoint, enters its next
model boundary, and replays the transcript summary in one call. The JIT adapter
can stage scheduler restoration locally until the host actually starts a run.
`complete_model_turn` submits the final assistant-message summary and advances
the model transition in one JIT RPC. The full message remains in TypeScript.
`complete_model_execution_plan` also combines the global and per-tool execution
modes with truncated-call handling, returning the model transition and one full
batch action: none, reject truncated calls, run sequentially, or run in
parallel. `complete_model_plan` remains available to older hosts.
`complete_tool_changes` selects how pending tool declarations are represented
and observes the selected message batch in one call. Message bodies and tool
definitions remain owned by TypeScript.
When the flags select an empty `keep` batch, the adapters recognize that the
operation cannot change Oche state and omit the runtime call entirely. This
removes one JIT RPC between an ordinary tool-result turn and its next model
request.
`begin_with_tool_changes` folds that selection into loop entry and full
transcript replay, so a fresh prompt reaches its first model boundary with one
JIT RPC.
`complete_tool_turn` similarly folds every tool-result observation, the batch
summary, and the tools transition into one RPC while retaining only the final
result summary and cumulative message count in Oche.
`complete_tool_outcomes` also receives the number of terminating tool results,
so Oche applies the all-results-must-terminate rule without a preceding policy
RPC.
Queue precedence and queue drain counts also cross this asynchronous boundary;
the JIT adapter does not duplicate those policies in TypeScript.
For the ordinary `PollQueues` path, `complete_queue_plan` combines queue
selection, retained observation, and the state transition into one JIT RPC.
It returns only the selected queue code and encoded machine state; queued
message objects remain in TypeScript.
`queue_poll_plan` completes that transition immediately when follow-up input
must remain untouched, including natural tool continuation and steering. It
requests a second host call only when the follow-up queue must actually be read.
When the host can inspect queues without consuming them,
`complete_queue_drain_plan` combines queue precedence, drain sizing, retained
observation, and the transition in one call. TypeScript then removes exactly
the selected batch.
For `Agent.continue()`, `continue_plan` combines transcript classification,
queue selection, continuation action, and drain sizing into one typed Oche
decision and one JIT RPC.
`begin_continue_with_tool_changes` additionally folds tool-declaration
selection, checkpoint entry, and transcript replay into that call when the
existing user or tool-result tail can continue directly.
`begin_continuation_with_tool_changes` accepts summarized candidates for the
direct, steering, and follow-up paths, then selects and enters the chosen path
atomically. Message objects and queue draining stay in TypeScript. A no-tool
continuation therefore uses three scheduler RPCs for both direct and
assistant-tail queued entry: entry, model completion, and queue completion.

The host executes one effect at a time:

| Code | Effect | Host responsibility |
| ---: | --- | --- |
| 1 | `RequestModel` | Stream one assistant response and count its tool calls. |
| 2 | `ExecuteTools` | Execute tool calls and report whether they naturally continue. |
| 3 | `PollQueues` | Read steering and follow-up queues, then report their counts. |
| 4 | `Done` | Finish the run. |
| 5 | `Invalid` | Reject a malformed state or an out-of-order transition. |

Messages, model streams, tool callbacks, and `AbortSignal` remain in TypeScript
for this slice. The state machine decides whether the next step is a model
request, tool execution, queue poll, or completion. This keeps the first
replacement small while making the control policy executable Oche code.

`runOcheAgent` is the effect interpreter. A host supplies `requestModel`,
`executeTools`, and `pollQueues`; the interpreter passes their outcomes into the
Oche machine as retained observations, then advances it until it reaches `Done`
or its `AbortSignal` is cancelled. The host
may keep pi's existing message, stream, and tool types behind those callbacks.
Tool-batch termination suppresses the automatic tool-result turn but still
allows steering, follow-up messages, or an explicit `continue` decision.
`toolBatchTerminates` also places the all-results-must-terminate rule in Oche;
the host passes only total and terminating-result counts across the boundary.
`queueTake` selects how many queued messages to drain for one-at-a-time and all
modes while the TypeScript host retains the messages themselves.
`toolsRunSequential` combines the global execution mode with per-tool
requirements; tool lookup and execution stay in the host.
`selectQueue` gives steering precedence over follow-up work without moving
message objects out of TypeScript.
`continueAction` defines how `Agent.continue()` handles an empty transcript,
an assistant tail, a continuable tail, and queued input. `continuePlan` applies
that policy atomically with queue precedence and queue mode.
`pollFollowUp` prevents follow-up queues from being consumed while a tool
result, steering message, or explicit end already determines the next action.
`toolChangeAction` decides whether pi keeps pending messages, rewrites their
system declaration, or inserts a new tool-loadout update after TypeScript
computes the concrete added and removed tool names.

Each `OcheAgentMachine` stores only its own encoded state and shares the
imported Dart-to-JavaScript code. The acceptance suite drives multiple machines
concurrently to verify that actor turns do not share mutable scheduler state.
`checkpoint()` returns a versioned, JSON-serializable envelope. Pass that
envelope back to `loadOcheAgentMachine` to validate and resume the scheduler
after a process or actor restart. The host must persist any in-flight provider
or tool data alongside it; the checkpoint covers Oche-owned scheduling state.
When injected into pi, the restored machine can enter at a `RequestModel`
boundary through `enterLoop`; other effects still require the standalone host
to restore its corresponding in-flight data.
The scheduler retains cumulative counters plus only the latest message and
transition summaries, so long-running actors do not accumulate a second copy
of their transcript. Format 1 checkpoints are migrated when restored.
The compiled module also exports ABI version 2. Both pi's public adapter and
the standalone driver reject a module with a different calling convention
before starting a machine; this is independent from checkpoint schema versions.
ABI 2 adds `enter`: a restored model boundary resumes in place, while a
completed checkpoint advances to a new model turn so a persisted Agent can
accept another prompt after process restart.
Both paths clear the derived message tail and count before pi replays the
persisted transcript, preventing duplicate observations after recovery.
At the high-level pi API, `Agent.checkpoint()` packages that scheduler envelope
with the transcript and pending steering/follow-up messages. Restore it through
`AgentOptions.checkpoint`, while the host supplies the current model, executable
tools, queue modes, and compiled Oche kernel.
pi-agent-core publishes the compiler output as its optional `oche-kernel`
subpath. `tests/run_pi_mono_acceptance.py` compares that vendored module byte for
byte with a fresh Oche build before running the cross-repository integration.
After changing the compiler or scheduler, pass `--update-vendored` to refresh
the pi artifact and run the same acceptance test.
Applications can import `OcheAgent` from the package's `oche` subpath to select
the bundled scheduler without wiring `compiledKernel` themselves. This entry
point also supplies the kernel when restoring a combined `AgentCheckpoint`.
The same `pi_core.oche` module is exercised through the Rust Dart VM binding by
`tests/run_oche_module_probe.py`. That probe loads the Kernel into a live JIT
isolate and drives `start`, `model_completed`, and `queues_completed` without
using the JavaScript backend.
`AgentOptions.onCheckpoint` is the stable actor persistence boundary: pi invokes
and awaits it after Oche reaches `Done` and `agent_end` listeners settle. The
cross-repository test queues follow-up work from that listener, persists it in
the callback, restarts the Agent, and consumes it through `continue()`.

Build and exercise it from the `oche` directory:

```sh
python3 tests/run_agent_machine_acceptance.py
python3 tests/run_jit_agent_machine_acceptance.py
```
