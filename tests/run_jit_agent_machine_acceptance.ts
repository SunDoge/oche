import assert from "node:assert/strict";
import { Effect } from "../agent/pi-core-driver.ts";
import { OcheJitModule, OcheJitRuntime } from "../agent/jit-core-driver.ts";

const [ocheExecutable, source, primitiveSource] = process.argv.slice(2);
if (!ocheExecutable || !source || !primitiveSource) {
	throw new Error("expected Oche executable, agent source, and primitive source paths");
}

const primitives = new OcheJitModule(ocheExecutable, primitiveSource);
try {
	assert.equal(await primitives.call<number>("scale", 1.5, 2.5), 3.75);
	assert.equal(await primitives.call<number>("later_float", 1.25), 1.75);
} finally {
	await primitives.close();
}

const runtime = await OcheJitRuntime.connect(ocheExecutable, source);
const machine = runtime.createMachine();
const second = runtime.createMachine();
const toolChanges = runtime.createMachine();
const toolEntry = runtime.createMachine();
const continueEntry = runtime.createMachine();
const queueDrain = runtime.createMachine();
const pid = runtime.pid;
try {
	const [firstStart, secondStart] = await Promise.all([
		machine.beginLoop([{ role: "user" }]),
		second.start(),
	]);
	assert.equal(firstStart.effect, Effect.RequestModel);
	assert.equal(secondStart.effect, Effect.RequestModel);
	await toolChanges.start();
	assert.equal(
		await toolChanges.completeToolChanges({
			changed: true,
			hasPending: false,
			pendingHadChanges: false,
			keepMessages: [{ role: "user" }],
			rewriteMessages: [{ role: "user" }],
			insertMessages: [{ role: "system" }, { role: "user" }],
		}),
		2,
	);
	assert.equal(toolChanges.state.message_count, 2);
	assert.deepEqual(toolChanges.state.messages, [{ role: "user", tool_calls: 0, failed: false }]);
	assert.equal(toolChanges.pid, pid);
	const enteredTools = await toolEntry.beginLoopWithToolChanges({
		changed: true,
		hasPending: false,
		pendingHadChanges: false,
		keepMessages: [{ role: "user" }],
		rewriteMessages: [{ role: "user" }],
		insertMessages: [{ role: "system" }, { role: "user" }],
	});
	assert.equal(enteredTools.action, 2);
	assert.equal(enteredTools.state.effect, Effect.RequestModel);
	assert.equal(enteredTools.state.message_count, 2);
	assert.equal(toolEntry.pid, pid);
	const continued = await continueEntry.beginContinueWithToolChanges({
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
	assert.equal(continued.action, 1);
	assert.equal(continued.toolAction, 2);
	assert.equal(continued.state?.effect, Effect.RequestModel);
	assert.equal(continued.state?.message_count, 2);
	assert.equal(continueEntry.pid, pid);
	const queuedContinue = runtime.createMachine();
	const callsBeforeQueuedContinue = runtime.callCount;
	const queuedEntry = await queuedContinue.beginContinueWithToolChanges({
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
	assert.equal(runtime.callCount - callsBeforeQueuedContinue, 1);
	assert.equal(queuedEntry.action, 3);
	assert.equal(queuedEntry.queue, 2);
	assert.equal(queuedEntry.take, 1);
	assert.equal(queuedEntry.toolAction, 2);
	assert.equal(queuedEntry.state?.message_count, 3);
	await queueDrain.start();
	await queueDrain.modelCompleted(0);
	const drained = await queueDrain.planQueueDrain({
		steering: 0,
		followUp: 3,
		steeringTakeAll: false,
		followUpTakeAll: true,
		forceContinue: false,
		forceEnd: false,
	});
	assert.equal(drained.queue, 2);
	assert.equal(drained.take, 3);
	assert.equal(drained.state.effect, Effect.RequestModel);
	assert.equal(queueDrain.pid, pid);
	const modelPlan = await machine.completeModelExecutionPlan(
		{
			role: "assistant",
			content: [{ type: "toolCall" }, { type: "toolCall" }],
			stopReason: "toolUse",
		},
		false,
		true,
	);
	assert.equal(modelPlan.state.effect, Effect.ExecuteTools);
	assert.equal(modelPlan.toolAction, 2);
	assert.equal(machine.state.message_count, 2);
	const truncated = runtime.createMachine();
	await truncated.start();
	const truncatedPlan = await truncated.completeModelExecutionPlan(
		{ role: "assistant", content: [{ type: "toolCall" }], stopReason: "length" },
		false,
		false,
	);
	assert.equal(truncatedPlan.state.effect, Effect.ExecuteTools);
	assert.equal(truncatedPlan.toolAction, 1);
	assert.equal(
		(
			await second.completeModel({
				role: "assistant",
				content: [{ type: "text", text: "done" }],
				stopReason: "stop",
			})
		).effect,
		Effect.PollQueues,
	);
	assert.equal(second.state.message_count, 1);
	const completion = await second.completeQueues({});
	assert.equal(completion.queue, 0);
	assert.equal(completion.state.effect, Effect.Done);
	assert.equal(completion.state.queues.observed, true);
	assert.equal(second.state.turn, 1);
	assert.deepEqual(
		await machine.planContinue({
			messages: [{ role: "assistant" }],
			steering: 0,
			followUp: 3,
			steeringTakeAll: false,
			followUpTakeAll: true,
		}),
		{ action: 3, queue: 2, take: 3 },
	);
	assert.equal(await machine.call<boolean>("tools_run_sequential", false, true), true);
	assert.equal(
		(
			await machine.completeToolOutcomes(
				[
					{ role: "toolResult", isError: false },
					{ role: "toolResult", isError: true },
				],
				{ total: 2, failed: 1, terminating: 1 },
			)
		).effect,
		Effect.PollQueues,
	);
	assert.equal(machine.state.message_count, 4);
	assert.deepEqual(machine.state.messages, [{ role: "toolResult", tool_calls: 0, failed: true }]);
	const queuePlan = await machine.planQueuePoll({});
	assert.equal(queuePlan.pollFollowUp, false);
	assert.equal(queuePlan.completion?.queue, 0);
	assert.equal(queuePlan.completion?.state.effect, Effect.RequestModel);
	assert.equal((await machine.modelCompleted(0)).effect, Effect.PollQueues);
	assert.equal((await machine.queuesCompleted()).effect, Effect.Done);
	assert.equal(machine.state.turn, 2);
	assert.equal(machine.pid, pid);
	assert.equal(second.pid, pid);

	const checkpoint = JSON.parse(JSON.stringify(machine.checkpoint()));
	machine.prepareSchedulerRestore({ abi: 2, state: checkpoint.state });
	assert.deepEqual(machine.checkpointScheduler(), { abi: 2, state: checkpoint.state });
	assert.equal((await machine.beginLoop([{ role: "user" }])).effect, Effect.RequestModel);
	assert.equal(machine.state.turn, 3);
	assert.equal(machine.state.message_count, 1);

	await assert.rejects(machine.call("missing_function"), /unknown Oche function/);
	assert.equal(await machine.call<number>("queue_take", 4, false), 1);
	assert.equal(machine.pid, pid);
} finally {
	await runtime.close();
}

console.log("TypeScript drove Oche agent transitions in one persistent Dart JIT process");

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
