import assert from "node:assert/strict";
import { Effect, loadOcheAgentMachine, runOcheAgent } from "../agent/pi-core-driver.ts";

const moduleUrl = process.argv[2];
if (!moduleUrl) throw new Error("expected compiled Oche module URL");

const plain = await loadOcheAgentMachine(moduleUrl);
assert.equal(plain.toolBatchTerminates(0, 0), false);
assert.equal(plain.toolBatchTerminates(2, 1), false);
assert.equal(plain.toolBatchTerminates(2, 2), true);
assert.equal(plain.queueTake(0, false), 0);
assert.equal(plain.queueTake(3, false), 1);
assert.equal(plain.queueTake(3, true), 3);
assert.equal(plain.toolsRunSequential(false, false), false);
assert.equal(plain.toolsRunSequential(true, false), true);
assert.equal(plain.toolsRunSequential(false, true), true);
assert.equal(plain.selectQueue(0, 0), 0);
assert.equal(plain.selectQueue(1, 1), 1);
assert.equal(plain.selectQueue(0, 1), 2);
assert.equal(plain.continueAction(0, 0), 0);
assert.equal(plain.continueAction(2, 0), 1);
assert.equal(plain.continueAction(1, 1), 2);
assert.equal(plain.continueAction(1, 2), 3);
assert.equal(plain.continueAction(1, 0), 4);
assert.deepEqual(
	plain.planContinue({
		messages: [{ role: "assistant" }],
		steering: 2,
		followUp: 1,
		steeringTakeAll: false,
		followUpTakeAll: true,
	}),
	{ action: 2, queue: 1, take: 1 },
);
assert.deepEqual(
	plain.planContinue({
		messages: [{ role: "assistant" }],
		steering: 0,
		followUp: 2,
		steeringTakeAll: false,
		followUpTakeAll: true,
	}),
	{ action: 3, queue: 2, take: 2 },
);
assert.equal(plain.toolChangeAction(false, false, false), 0);
assert.equal(plain.toolChangeAction(false, true, false), 0);
assert.equal(plain.toolChangeAction(false, true, true), 1);
assert.equal(plain.toolChangeAction(true, true, false), 1);
assert.equal(plain.toolChangeAction(true, false, false), 2);

const atomicToolChanges = await loadOcheAgentMachine(moduleUrl);
atomicToolChanges.start();
assert.equal(
	atomicToolChanges.completeToolChanges({
		changed: true,
		hasPending: false,
		pendingHadChanges: false,
		keepMessages: [{ role: "user" }],
		rewriteMessages: [{ role: "user" }],
		insertMessages: [{ role: "system" }, { role: "user" }],
	}),
	2,
);
assert.equal(atomicToolChanges.state.message_count, 2);
assert.deepEqual(atomicToolChanges.state.messages, [
	{ role: "user", tool_calls: 0, failed: false },
]);
assert.equal(
	atomicToolChanges.completeToolChanges({
		changed: true,
		hasPending: true,
		pendingHadChanges: false,
		keepMessages: [{ role: "system" }, { role: "user" }],
		rewriteMessages: [
			{ role: "system" },
			{ role: "assistant", content: [{ type: "toolCall" }], stopReason: "error" },
		],
		insertMessages: [{ role: "system" }, { role: "user" }],
	}),
	1,
);
assert.equal(atomicToolChanges.state.message_count, 4);
assert.deepEqual(atomicToolChanges.state.messages, [
	{ role: "assistant", tool_calls: 1, failed: true },
]);

const atomicToolEntry = await loadOcheAgentMachine(moduleUrl);
const toolEntry = atomicToolEntry.beginLoopWithToolChanges({
	changed: true,
	hasPending: false,
	pendingHadChanges: false,
	keepMessages: [{ role: "user" }],
	rewriteMessages: [{ role: "user" }],
	insertMessages: [{ role: "system" }, { role: "user" }],
});
assert.equal(toolEntry.action, 2);
assert.equal(toolEntry.state.effect, Effect.RequestModel);
assert.equal(toolEntry.state.message_count, 2);
assert.deepEqual(toolEntry.state.messages, [{ role: "user", tool_calls: 0, failed: false }]);
const atomicContinueEntry = await loadOcheAgentMachine(moduleUrl);
const continueEntry = atomicContinueEntry.beginContinueWithToolChanges({
	messages: [{ role: "user" }],
	steering: 0,
	followUp: 0,
	steeringTakeAll: false,
	followUpTakeAll: true,
	toolChanges: {
		changed: true,
		hasPending: false,
		pendingHadChanges: false,
		keepMessages: [{ role: "user" }],
		rewriteMessages: [{ role: "user" }],
		insertMessages: [{ role: "system" }, { role: "user" }],
	},
});
assert.equal(continueEntry.action, 1);
assert.equal(continueEntry.toolAction, 2);
assert.equal(continueEntry.state?.effect, Effect.RequestModel);
assert.equal(continueEntry.state?.message_count, 2);
const queuedContinueEntry = await loadOcheAgentMachine(moduleUrl);
const queuedEntry = queuedContinueEntry.beginContinueWithToolChanges({
	messages: [{ role: "assistant" }],
	steering: 0,
	followUp: 2,
	steeringTakeAll: false,
	followUpTakeAll: false,
	toolChanges: unchanged([{ role: "assistant" }]),
	steeringToolChanges: unchanged([{ role: "assistant" }]),
	followUpToolChanges: {
		changed: true,
		hasPending: false,
		pendingHadChanges: false,
		keepMessages: [{ role: "assistant" }, { role: "user" }],
		rewriteMessages: [{ role: "assistant" }, { role: "user" }],
		insertMessages: [{ role: "assistant" }, { role: "system" }, { role: "user" }],
	},
});
assert.equal(queuedEntry.action, 3);
assert.equal(queuedEntry.queue, 2);
assert.equal(queuedEntry.take, 1);
assert.equal(queuedEntry.toolAction, 2);
assert.equal(queuedEntry.state?.effect, Effect.RequestModel);
assert.equal(queuedEntry.state?.message_count, 3);
assert.equal(plain.start().effect, Effect.RequestModel);
assert.equal(plain.pollFollowUp(0, false), true);
assert.equal(plain.pollFollowUp(1, false), false);
assert.equal(plain.pollFollowUp(0, true), false);
plain.observeMessages([{ role: "system" }, { role: "user" }]);
assert.equal(plain.modelCompleted(0).effect, Effect.PollQueues);
assert.equal(plain.queuesCompleted().effect, Effect.Done);
assert.equal(plain.state.schema, 2);
assert.equal(plain.state.transition_count, 3);
assert.deepEqual(plain.state.history, ["queues"]);
assert.equal(plain.state.message_count, 2);
assert.deepEqual(plain.state.messages, [
	{ role: "user", tool_calls: 0, failed: false },
]);
assert.equal(plain.observedMessageCount(), 2);
assert.equal(plain.tailMessageKind(), 3);
assert.equal(plain.classifyMessageTail([]), 0);
assert.equal(plain.classifyMessageTail([{ role: "system" }]), 0);
assert.equal(plain.classifyMessageTail([{ role: "assistant" }]), 1);
assert.equal(plain.classifyMessageTail([{ role: "user" }]), 2);

const atomicBegin = await loadOcheAgentMachine(moduleUrl);
assert.equal(atomicBegin.beginLoop([{ role: "system" }, { role: "user" }]).effect, Effect.RequestModel);
assert.equal(atomicBegin.state.message_count, 2);
assert.deepEqual(atomicBegin.state.messages, [{ role: "user", tool_calls: 0, failed: false }]);

const observedToolTurn = await loadOcheAgentMachine(moduleUrl);
observedToolTurn.start();
observedToolTurn.observeMessages([
	{
		role: "assistant",
		content: [{ type: "toolCall" }, { type: "toolCall" }],
		stopReason: "stop",
	},
]);
assert.equal(observedToolTurn.advanceModel().effect, Effect.ExecuteTools);

const observedFailure = await loadOcheAgentMachine(moduleUrl);
observedFailure.start();
observedFailure.observeMessages([{ role: "assistant", content: [], stopReason: "error" }]);
assert.equal(observedFailure.advanceModel().effect, Effect.Done);

const atomicModel = await loadOcheAgentMachine(moduleUrl);
atomicModel.start();
const modelPlan = atomicModel.completeModelExecutionPlan(
	{
		role: "assistant",
		content: [{ type: "toolCall" }, { type: "toolCall" }],
		stopReason: "stop",
	},
	false,
	true,
);
assert.equal(modelPlan.state.effect, Effect.ExecuteTools);
assert.equal(modelPlan.toolAction, 2);
assert.equal(atomicModel.state.message_count, 1);
assert.deepEqual(atomicModel.state.messages, [{ role: "assistant", tool_calls: 2, failed: false }]);

const truncatedModel = await loadOcheAgentMachine(moduleUrl);
truncatedModel.start();
const truncatedPlan = truncatedModel.completeModelExecutionPlan(
	{ role: "assistant", content: [{ type: "toolCall" }], stopReason: "length" },
	false,
	false,
);
assert.equal(truncatedPlan.state.effect, Effect.ExecuteTools);
assert.equal(truncatedPlan.toolAction, 1);
assert.equal(
	atomicModel.completeToolOutcomes(
		[
			{ role: "toolResult", isError: false },
			{ role: "toolResult", isError: true },
		],
		{ total: 2, failed: 1, terminating: 1 },
	).effect,
	Effect.PollQueues,
);
assert.equal(atomicModel.state.message_count, 3);
assert.deepEqual(atomicModel.state.messages, [{ role: "toolResult", tool_calls: 0, failed: true }]);
assert.deepEqual(atomicModel.state.tool_batch, {
	observed: true,
	total: 2,
	failed: 1,
	terminate: false,
});
assert.equal(atomicModel.state.must_continue, true);

const observedTermination = await loadOcheAgentMachine(moduleUrl);
observedTermination.start();
observedTermination.modelCompleted(1);
observedTermination.observeToolBatch({ total: 1, failed: 0, terminate: true });
assert.equal(observedTermination.advanceTools().effect, Effect.PollQueues);
assert.equal(observedTermination.state.must_continue, false);
assert.deepEqual(observedTermination.state.tool_batch, {
	observed: true,
	total: 1,
	failed: 0,
	terminate: true,
});
observedTermination.queuesCompleted({ forceContinue: true });
observedTermination.modelCompleted(1);
assert.equal(observedTermination.toolsCompleted(false).must_continue, true);
assert.equal(observedTermination.pollFollowUp(0, false), false);

const observedQueue = await loadOcheAgentMachine(moduleUrl);
observedQueue.start();
observedQueue.modelCompleted(0);
observedQueue.observeQueues({ steeringMessages: 1 });
assert.deepEqual(observedQueue.state.queues, {
	observed: true,
	steering: 1,
	follow_up: 0,
	force_continue: false,
	force_end: false,
});
assert.equal(observedQueue.advanceQueues().effect, Effect.RequestModel);
assert.equal(observedQueue.state.queues.observed, false);
observedQueue.modelCompleted(0);
assert.equal(observedQueue.queuesCompleted().effect, Effect.Done);

const atomicQueue = await loadOcheAgentMachine(moduleUrl);
atomicQueue.start();
atomicQueue.modelCompleted(0);
const queuePlan = atomicQueue.planQueuePoll({ steeringMessages: 1 });
assert.equal(queuePlan.pollFollowUp, false);
assert.ok(queuePlan.completion);
const queueCompletion = queuePlan.completion;
assert.equal(queueCompletion.queue, 1);
assert.equal(queueCompletion.state.effect, Effect.RequestModel);
assert.equal(queueCompletion.state.queues.observed, false);

const followUpPoll = await loadOcheAgentMachine(moduleUrl);
followUpPoll.start();
followUpPoll.modelCompleted(0);
assert.deepEqual(followUpPoll.planQueuePoll({}), { pollFollowUp: true });
assert.equal(followUpPoll.state.effect, Effect.PollQueues);

const atomicDrain = await loadOcheAgentMachine(moduleUrl);
atomicDrain.start();
atomicDrain.modelCompleted(0);
const drainPlan = atomicDrain.planQueueDrain({
	steering: 3,
	followUp: 2,
	steeringTakeAll: false,
	followUpTakeAll: true,
	forceContinue: false,
	forceEnd: false,
});
assert.equal(drainPlan.queue, 1);
assert.equal(drainPlan.take, 1);
assert.equal(drainPlan.state.effect, Effect.RequestModel);
assert.equal(drainPlan.state.queues.observed, false);

const naturalDrain = await loadOcheAgentMachine(moduleUrl);
naturalDrain.start();
naturalDrain.modelCompleted(1);
naturalDrain.toolsCompleted(false);
const naturalDrainPlan = naturalDrain.planQueueDrain({
	steering: 0,
	followUp: 3,
	steeringTakeAll: false,
	followUpTakeAll: true,
	forceContinue: false,
	forceEnd: false,
});
assert.equal(naturalDrainPlan.queue, 0);
assert.equal(naturalDrainPlan.take, 0);
assert.equal(naturalDrainPlan.state.effect, Effect.RequestModel);
assert.equal(naturalDrainPlan.state.queues.follow_up, 0);

const forcedDrainEnd = await loadOcheAgentMachine(moduleUrl);
forcedDrainEnd.start();
forcedDrainEnd.modelCompleted(0);
const forcedDrainPlan = forcedDrainEnd.planQueueDrain({
	steering: 3,
	followUp: 2,
	steeringTakeAll: true,
	followUpTakeAll: true,
	forceContinue: false,
	forceEnd: true,
});
assert.equal(forcedDrainPlan.queue, 0);
assert.equal(forcedDrainPlan.take, 0);
assert.equal(forcedDrainPlan.state.effect, Effect.Done);

const beforeRestart = await loadOcheAgentMachine(moduleUrl);
beforeRestart.start();
beforeRestart.observeMessages([{ role: "user" }, { role: "assistant" }]);
beforeRestart.modelCompleted(0);
const checkpoint = JSON.parse(JSON.stringify(beforeRestart.checkpoint()));
assert.equal(checkpoint.format, 2);
const afterRestart = await loadOcheAgentMachine(moduleUrl, checkpoint);
assert.deepEqual(afterRestart.state, beforeRestart.state);
assert.equal(afterRestart.queuesCompleted({ forceContinue: true }).effect, Effect.RequestModel);
assert.equal(afterRestart.state.turn, 2);
assert.throws(
	() => afterRestart.restore({ format: 3, state: checkpoint.state } as never),
	/Unsupported Oche agent checkpoint format/,
);

const atomicRestore = await loadOcheAgentMachine(moduleUrl);
const atomicCheckpoint = afterRestart.checkpoint();
assert.equal(
	atomicRestore.restoreBegin(atomicCheckpoint, [
		{ role: "system" },
		{ role: "user", content: "continue" },
	]).effect,
	Effect.RequestModel,
);
assert.equal(atomicRestore.state.turn, 2);
assert.equal(atomicRestore.state.message_count, 2);
assert.deepEqual(atomicRestore.state.messages, [
	{ role: "user", tool_calls: 0, failed: false },
]);

const unsupportedAtomicRestore = await loadOcheAgentMachine(moduleUrl);
const unsupportedState = JSON.stringify({ ...JSON.parse(atomicCheckpoint.state), schema: 3 });
assert.throws(
	() => unsupportedAtomicRestore.restoreBegin({ format: 2, state: unsupportedState }, []),
	/unsupported machine schema/,
);

const corruptCheckpoint = await loadOcheAgentMachine(moduleUrl);
assert.throws(
	() => corruptCheckpoint.restore({ format: 2, state: '{"phase":"bad"}' }),
	/Invalid Oche machine state/,
);

const legacyCheckpoint = {
	format: 1 as const,
	state: JSON.stringify({
		phase: 3,
		turn: 1,
		must_continue: false,
		effect: Effect.PollQueues,
		error: "",
		history: ["start", "model"],
		messages: [
			{ role: "user", tool_calls: 0, failed: false },
			{ role: "assistant", tool_calls: 0, failed: false },
		],
		tool_batch: { observed: false, total: 0, failed: 0, terminate: false },
		queues: { observed: false, steering: 0, follow_up: 0, force_continue: false, force_end: false },
	}),
};
const migrated = await loadOcheAgentMachine(moduleUrl, legacyCheckpoint);
assert.equal(migrated.state.schema, 2);
assert.equal(migrated.state.transition_count, 2);
assert.equal(migrated.state.message_count, 2);
assert.deepEqual(migrated.state.history, ["model"]);
assert.deepEqual(migrated.state.messages, [
	{ role: "assistant", tool_calls: 0, failed: false },
]);

const bounded = await loadOcheAgentMachine(moduleUrl);
bounded.start();
for (let index = 0; index < 1000; index += 1) {
	bounded.observeMessages([{ role: index % 2 === 0 ? "user" : "assistant" }]);
}
assert.equal(bounded.state.message_count, 1000);
assert.equal(bounded.state.messages.length, 1);
assert.ok(bounded.checkpoint().state.length < 1000);

const boundedTransitions = await loadOcheAgentMachine(moduleUrl);
boundedTransitions.start();
for (let turn = 0; turn < 100; turn += 1) {
	boundedTransitions.modelCompleted(1);
	boundedTransitions.toolsCompleted();
	boundedTransitions.queuesCompleted();
}
assert.equal(boundedTransitions.state.transition_count, 301);
assert.equal(boundedTransitions.state.history.length, 1);
assert.ok(boundedTransitions.checkpoint().state.length < 1000);

const resumable = await loadOcheAgentMachine(moduleUrl);
resumable.start();
resumable.modelCompleted(0);
const resumed = await runOcheAgent(
	await loadOcheAgentMachine(moduleUrl, resumable.checkpoint()),
	{
		async requestModel() {
			throw new Error("resumed queue poll must not request a model");
		},
		async executeTools() {
			throw new Error("resumed queue poll must not execute tools");
		},
		async pollQueues() {
			return {};
		},
	},
);
assert.equal(resumed.reason, "done");
assert.equal(resumed.state.transition_count, 3);
assert.deepEqual(resumed.state.history, ["queues"]);

const tools = await loadOcheAgentMachine(moduleUrl);
assert.deepEqual(
	[tools.start().effect, tools.modelCompleted(2).effect, tools.toolsCompleted().effect],
	[Effect.RequestModel, Effect.ExecuteTools, Effect.PollQueues],
);
assert.equal(tools.queuesCompleted({ steeringMessages: 1 }).effect, Effect.RequestModel);
assert.equal(tools.state.turn, 2);
assert.equal(tools.modelCompleted(0).effect, Effect.PollQueues);
assert.equal(tools.queuesCompleted({ followUpMessages: 1 }).effect, Effect.RequestModel);
assert.equal(tools.state.turn, 3);
assert.equal(tools.modelCompleted(0, true).effect, Effect.Done);
assert.equal(tools.state.transition_count, 7);
assert.deepEqual(tools.state.history, ["model"]);

const terminated = await loadOcheAgentMachine(moduleUrl);
terminated.start();
terminated.modelCompleted(1);
assert.equal(terminated.toolsCompleted(true).effect, Effect.PollQueues);
assert.equal(terminated.queuesCompleted().effect, Effect.Done);

const terminatedButSteered = await loadOcheAgentMachine(moduleUrl);
terminatedButSteered.start();
terminatedButSteered.modelCompleted(1);
terminatedButSteered.toolsCompleted(true);
assert.equal(terminatedButSteered.queuesCompleted({ steeringMessages: 1 }).effect, Effect.RequestModel);

const invalid = await loadOcheAgentMachine(moduleUrl);
invalid.start();
assert.throws(() => invalid.toolsCompleted(), /requires the tools phase/);

const driven = await loadOcheAgentMachine(moduleUrl);
const calls: string[] = [];
const scriptedTurns = [
	{ id: "tool-turn", toolCalls: 2 },
	{ id: "steered-turn", toolCalls: 0 },
	{ id: "follow-up-turn", toolCalls: 0 },
];
let queuePoll = 0;
const run = await runOcheAgent(driven, {
	async requestModel(state) {
		const turn = scriptedTurns.shift();
		if (!turn) throw new Error("unexpected model request");
		calls.push(`model:${state.turn}:${turn.id}`);
		return { value: turn, toolCalls: turn.toolCalls };
	},
	async executeTools(turn) {
		calls.push(`tools:${turn.id}`);
		return {};
	},
	async pollQueues() {
		queuePoll += 1;
		calls.push(`queues:${queuePoll}`);
		if (queuePoll === 1) return { steeringMessages: 1 };
		if (queuePoll === 2) return { followUpMessages: 1 };
		return {};
	},
});
assert.equal(run.reason, "done");
assert.deepEqual(calls, [
	"model:1:tool-turn",
	"tools:tool-turn",
	"queues:1",
	"model:2:steered-turn",
	"queues:2",
	"model:3:follow-up-turn",
	"queues:3",
]);
assert.equal(run.state.message_count, 3);
assert.deepEqual(run.state.messages, [
	{ role: "assistant", tool_calls: 0, failed: false },
]);
assert.equal(run.state.tool_batch.observed, true);
assert.equal(run.state.queues.observed, true);

const controller = new AbortController();
controller.abort();
const aborted = await runOcheAgent(await loadOcheAgentMachine(moduleUrl), {
	async requestModel() {
		throw new Error("aborted run must not call host");
	},
	async executeTools() {
		throw new Error("aborted run must not call host");
	},
	async pollQueues() {
		throw new Error("aborted run must not call host");
	},
}, controller.signal);
assert.equal(aborted.reason, "aborted");

const concurrentToolCalls: string[] = [];
const runConcurrent = async (name: string, useTool: boolean) => {
	let requests = 0;
	return runOcheAgent(await loadOcheAgentMachine(moduleUrl), {
		async requestModel(state) {
			requests += 1;
			await Promise.resolve();
			concurrentToolCalls.push(`${name}:model:${state.turn}`);
			return { value: name, toolCalls: useTool && requests === 1 ? 1 : 0 };
		},
		async executeTools() {
			await Promise.resolve();
			concurrentToolCalls.push(`${name}:tools`);
			return {};
		},
		async pollQueues() {
			await Promise.resolve();
			return {};
		},
	});
};
const [shortRun, toolRun] = await Promise.all([
	runConcurrent("short", false),
	runConcurrent("tool", true),
]);
assert.equal(shortRun.state.turn, 1);
assert.equal(toolRun.state.turn, 2);
assert.deepEqual(concurrentToolCalls.toSorted(), [
	"short:model:1",
	"tool:model:1",
	"tool:model:2",
	"tool:tools",
]);

console.log("TypeScript-driven Oche agent state machine passed");

function unchanged(messages: readonly unknown[]) {
	return {
		changed: false,
		hasPending: false,
		pendingHadChanges: false,
		keepMessages: messages,
		rewriteMessages: messages,
		insertMessages: messages,
	};
}
