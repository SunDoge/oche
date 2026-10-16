import {
	createAssistantMessageEventStream,
	type AssistantMessage,
	type Message,
	type Model,
} from "@earendil-works/pi-ai";
import {
	Agent,
	AGENT_CHECKPOINT_VERSION,
	agentLoop,
	COMPILED_AGENT_LOOP_ABI_VERSION,
	CompiledAgentLoopStateMachine,
	type AgentMessage,
	type AgentTool,
	type CompiledAgentLoopKernel,
} from "@earendil-works/pi-agent-core";
import { Type } from "typebox";
import { describe, expect, it } from "vitest";
import { OcheJitRuntime } from "../agent/jit-core-driver.ts";
import type { MachineState } from "../agent/pi-core-driver.ts";

const usage = {
	input: 0,
	output: 0,
	cacheRead: 0,
	cacheWrite: 0,
	totalTokens: 0,
	cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0, total: 0 },
};

function assistant(content: AssistantMessage["content"]): AssistantMessage {
	return {
		role: "assistant",
		content,
		api: "openai-responses",
		provider: "openai",
		model: "mock",
		usage,
		stopReason: "stop",
		timestamp: Date.now(),
	};
}

const model: Model<"openai-responses"> = {
	id: "mock",
	name: "mock",
	api: "openai-responses",
	provider: "openai",
	baseUrl: "https://example.invalid",
	reasoning: false,
	input: ["text"],
	cost: { input: 0, output: 0, cacheRead: 0, cacheWrite: 0 },
	contextWindow: 8192,
	maxTokens: 2048,
};

describe("compiled Oche pi scheduler", () => {
	it("drives the high-level Agent through one persistent Dart JIT process", async () => {
		const executable = process.env.OCHE_EXECUTABLE;
		const source = process.env.OCHE_SOURCE;
		if (!executable || !source) throw new Error("OCHE_EXECUTABLE and OCHE_SOURCE are required");
		const runtime = await OcheJitRuntime.connect(executable, source);
		const stateMachine = runtime.createMachine();
		const pid = runtime.pid;
		try {
			let requests = 0;
			let executions = 0;
			const parameters = Type.Object({ value: Type.String() });
			const tool: AgentTool<typeof parameters> = {
				name: "echo",
				label: "Echo",
				description: "Echo a string",
				parameters,
				async execute(_id, params) {
					executions += 1;
					return { content: [{ type: "text", text: params.value }], details: {} };
				},
			};
			const callsBeforePrompt = runtime.callCount;
			const agent = new Agent({
				initialState: { model, tools: [tool] },
				stateMachine,
				streamFn: () => {
					requests += 1;
					const response = createAssistantMessageEventStream();
					queueMicrotask(() => {
						response.push({
							type: "done",
							reason: "stop",
							message:
								requests === 1
									? assistant([
											{ type: "toolCall", id: "call-1", name: "echo", arguments: { value: "Dart JIT" } },
										])
									: assistant([{ type: "text", text: "done" }]),
						});
					});
					return response;
				},
			});

			await agent.prompt("jit");
			expect(runtime.callCount - callsBeforePrompt).toBe(6);
			expect(requests).toBe(2);
			expect(executions).toBe(1);
			expect(agent.state.messages.map((message) => message.role)).toEqual([
				"system",
				"user",
				"assistant",
				"toolResult",
				"assistant",
			]);
			expect(stateMachine.state.effect).toBe(4);
			expect(stateMachine.pid).toBe(pid);
		} finally {
			await runtime.close();
		}
	});

	it("enters a direct Agent continuation in one scheduler call", async () => {
		const executable = process.env.OCHE_EXECUTABLE;
		const source = process.env.OCHE_SOURCE;
		if (!executable || !source) throw new Error("OCHE_EXECUTABLE and OCHE_SOURCE are required");
		const runtime = await OcheJitRuntime.connect(executable, source);
		const stateMachine = runtime.createMachine();
		try {
			const agent = new Agent({
				initialState: {
					model,
					messages: [{ role: "user", content: "continue", timestamp: Date.now() }],
				},
				stateMachine,
				streamFn: () => {
					const response = createAssistantMessageEventStream();
					queueMicrotask(() => {
						response.push({
							type: "done",
							reason: "stop",
							message: assistant([{ type: "text", text: "continued" }]),
						});
					});
					return response;
				},
			});
			const callsBeforeContinue = runtime.callCount;

			await agent.continue();

			expect(runtime.callCount - callsBeforeContinue).toBe(3);
			expect(agent.state.messages.map((message) => message.role)).toEqual(["user", "assistant"]);
			expect(stateMachine.state.effect).toBe(4);
		} finally {
			await runtime.close();
		}
	});

	it("persists and restores a high-level Agent through the shared Dart JIT", async () => {
		const executable = process.env.OCHE_EXECUTABLE;
		const source = process.env.OCHE_SOURCE;
		if (!executable || !source) throw new Error("OCHE_EXECUTABLE and OCHE_SOURCE are required");
		const runtime = await OcheJitRuntime.connect(executable, source);
		const pid = runtime.pid;
		const respond = (text: string) => {
			const response = createAssistantMessageEventStream();
			queueMicrotask(() => {
				response.push({
					type: "done",
					reason: "stop",
					message: assistant([{ type: "text", text }]),
				});
			});
			return response;
		};
		try {
			let persisted: ReturnType<Agent["checkpoint"]> | undefined;
			const firstMachine = runtime.createMachine();
			const first = new Agent({
				initialState: { model },
				stateMachine: firstMachine,
				streamFn: () => respond("first"),
				onCheckpoint: (checkpoint) => {
					persisted = JSON.parse(JSON.stringify(checkpoint)) as typeof checkpoint;
				},
			});
			first.subscribe((event) => {
				if (event.type === "agent_end") {
					first.followUp({ role: "user", content: "two", timestamp: Date.now() });
				}
			});
			await first.prompt("one");
			if (!persisted) throw new Error("automatic Agent checkpoint is required");
			expect(persisted.scheduler?.abi).toBe(COMPILED_AGENT_LOOP_ABI_VERSION);

			const secondMachine = runtime.createMachine();
			const second = new Agent({
				initialState: { model },
				checkpoint: persisted,
				stateMachine: secondMachine,
				streamFn: () => respond("second"),
			});
			const callsBeforeContinue = runtime.callCount;
			await second.continue();
			expect(runtime.callCount - callsBeforeContinue).toBe(3);

			expect(second.state.messages.map((message) => message.role)).toEqual([
				"user",
				"assistant",
				"user",
				"assistant",
			]);
			expect(secondMachine.state.turn).toBe(2);
			expect(secondMachine.pid).toBe(pid);
		} finally {
			await runtime.close();
		}
	});

	it("accepts a kernel directly in the low-level loop", async () => {
		const moduleUrl = process.env.OCHE_PI_CORE_URL;
		if (!moduleUrl) throw new Error("OCHE_PI_CORE_URL is required");
		const imported = (await import(moduleUrl)) as { default: CompiledAgentLoopKernel };
		let requests = 0;
		const stream = agentLoop(
			[{ role: "user", content: "direct", timestamp: Date.now() }],
			{ messages: [], tools: [] },
			{
				model,
				convertToLlm: (messages: AgentMessage[]) => messages as Message[],
				compiledKernel: imported.default,
			},
			undefined,
			() => {
				requests += 1;
				const response = createAssistantMessageEventStream();
				queueMicrotask(() => {
					response.push({
						type: "done",
						reason: "stop",
						message: assistant([{ type: "text", text: "direct" }]),
					});
				});
				return response;
			},
		);

		const messages = await stream.result();
		expect(requests).toBe(1);
		expect(messages.map((message) => message.role)).toEqual(["user", "assistant"]);
	});

	it("enters pi from a restored model-request boundary", async () => {
		const moduleUrl = process.env.OCHE_PI_CORE_URL;
		if (!moduleUrl) throw new Error("OCHE_PI_CORE_URL is required");
		const imported = (await import(moduleUrl)) as { default: CompiledAgentLoopKernel };
		const seed = new CompiledAgentLoopStateMachine<MachineState>(imported.default);
		seed.start();
		seed.observeMessages([assistant([{ type: "text", text: "seed" }])]);
		seed.advanceModel();
		seed.observeQueues({ steeringMessages: 1 });
		seed.advanceQueues();
		const stateMachine = new CompiledAgentLoopStateMachine<MachineState>(imported.default);
		stateMachine.restore(seed.checkpoint());

		const stream = agentLoop(
			[{ role: "user", content: "resume", timestamp: Date.now() }],
			{ messages: [], tools: [] },
			{
				model,
				convertToLlm: (messages: AgentMessage[]) => messages as Message[],
				stateMachine,
			},
			undefined,
			() => {
				const response = createAssistantMessageEventStream();
				queueMicrotask(() => {
					response.push({
						type: "done",
						reason: "stop",
						message: assistant([{ type: "text", text: "resumed" }]),
					});
				});
				return response;
			},
		);

		await stream.result();
		expect(stateMachine.state.turn).toBe(2);
		expect(stateMachine.state.transition_count).toBe(5);
	});

	it("restores a scheduler checkpoint through AgentOptions", async () => {
		const moduleUrl = process.env.OCHE_PI_CORE_URL;
		if (!moduleUrl) throw new Error("OCHE_PI_CORE_URL is required");
		const imported = (await import(moduleUrl)) as { default: CompiledAgentLoopKernel };
		const seed = new CompiledAgentLoopStateMachine<MachineState>(imported.default);
		seed.start();
		seed.observeMessages([assistant([{ type: "text", text: "seed" }])]);
		seed.advanceModel();
		seed.observeQueues({ steeringMessages: 1 });
		seed.advanceQueues();
		let requests = 0;
		const agent = new Agent({
			initialState: { model },
			compiledKernel: imported.default,
			compiledCheckpoint: seed.checkpoint(),
			streamFn: () => {
				requests += 1;
				const response = createAssistantMessageEventStream();
				queueMicrotask(() => {
					response.push({
						type: "done",
						reason: "stop",
						message: assistant([{ type: "text", text: "restored" }]),
					});
				});
				return response;
			},
		});

		await agent.prompt("resume");

		expect(requests).toBe(1);
		const checkpoint = agent.checkpointScheduler();
		expect(checkpoint?.abi).toBe(COMPILED_AGENT_LOOP_ABI_VERSION);
		expect((JSON.parse(checkpoint?.state ?? "{}") as MachineState).turn).toBe(2);
	});

	it("reopens a completed Agent after persistence", async () => {
		const moduleUrl = process.env.OCHE_PI_CORE_URL;
		if (!moduleUrl) throw new Error("OCHE_PI_CORE_URL is required");
		const imported = (await import(moduleUrl)) as { default: CompiledAgentLoopKernel };
		const respond = (text: string) => {
			const response = createAssistantMessageEventStream();
			queueMicrotask(() => {
				response.push({
					type: "done",
					reason: "stop",
					message: assistant([{ type: "text", text }]),
				});
			});
			return response;
		};
		let persisted: ReturnType<Agent["checkpoint"]> | undefined;
		const first = new Agent({
			initialState: { model },
			compiledKernel: imported.default,
			streamFn: () => respond("first"),
			onCheckpoint: (checkpoint) => {
				persisted = JSON.parse(JSON.stringify(checkpoint)) as typeof checkpoint;
			},
		});
		first.subscribe((event) => {
			if (event.type === "agent_end") {
				first.followUp({ role: "user", content: "two", timestamp: Date.now() });
			}
		});
		await first.prompt("one");
		if (!persisted) throw new Error("automatic Agent checkpoint is required");
		expect(persisted.version).toBe(AGENT_CHECKPOINT_VERSION);
		expect(persisted.scheduler?.abi).toBe(COMPILED_AGENT_LOOP_ABI_VERSION);
		expect(persisted.followUpMessages).toHaveLength(1);

		const second = new Agent({
			initialState: { model },
			checkpoint: persisted,
			compiledKernel: imported.default,
			streamFn: () => respond("second"),
		});
		await second.continue();

		expect(second.state.messages.map((message) => message.role)).toEqual([
			"user",
			"assistant",
			"user",
			"assistant",
		]);
		const resumed = JSON.parse(second.checkpointScheduler()?.state ?? "{}") as MachineState;
		expect(resumed.turn).toBe(2);
		expect(resumed.message_count).toBe(4);
	});

	it("drives pi-agent-core through a tool turn", async () => {
		const moduleUrl = process.env.OCHE_PI_CORE_URL;
		if (!moduleUrl) throw new Error("OCHE_PI_CORE_URL is required");
		const imported = (await import(moduleUrl)) as { default: CompiledAgentLoopKernel };
		const stateMachine = new CompiledAgentLoopStateMachine<MachineState>(imported.default);
		let requests = 0;
		let executions = 0;
		const parameters = Type.Object({ value: Type.String() });
		const tool: AgentTool<typeof parameters> = {
			name: "echo",
			label: "Echo",
			description: "Echo a string",
			parameters,
			async execute(_id, params) {
				executions += 1;
				return { content: [{ type: "text", text: params.value }], details: {} };
			},
		};

		const stream = agentLoop(
			[{ role: "user", content: "run", timestamp: Date.now() }],
			{ messages: [], tools: [tool] },
			{
				model,
				convertToLlm: (messages: AgentMessage[]) => messages as Message[],
				stateMachine,
			},
			undefined,
			() => {
				requests += 1;
				const response = createAssistantMessageEventStream();
				queueMicrotask(() => {
					response.push({
						type: "done",
						reason: "stop",
						message:
							requests === 1
								? assistant([
										{
											type: "toolCall",
											id: "call-1",
											name: "echo",
											arguments: { value: "ok" },
										},
									])
								: assistant([{ type: "text", text: "done" }]),
					});
				});
				return response;
			},
		);

		const messages = await stream.result();
		expect(requests).toBe(2);
		expect(executions).toBe(1);
		expect(messages.map((message) => message.role)).toEqual([
			"system",
			"user",
			"assistant",
			"toolResult",
			"assistant",
		]);
		expect(stateMachine.state.message_count).toBe(messages.length);
		expect(stateMachine.state.messages).toEqual([
			{ role: "assistant", tool_calls: 0, failed: false },
		]);
		expect(stateMachine.state.tool_batch).toEqual({
			observed: true,
			total: 1,
			failed: 0,
			terminate: false,
		});
		expect(stateMachine.state.queues.observed).toBe(true);
	});

	it("classifies an assistant transcript tail before Agent.continue()", async () => {
		const moduleUrl = process.env.OCHE_PI_CORE_URL;
		if (!moduleUrl) throw new Error("OCHE_PI_CORE_URL is required");
		const imported = (await import(moduleUrl)) as { default: CompiledAgentLoopKernel };
		let requests = 0;
		const agent = new Agent({
			initialState: {
				model,
				messages: [
					{ role: "user", content: "initial", timestamp: Date.now() },
					assistant([{ type: "text", text: "ready" }]),
				],
			},
			compiledKernel: imported.default,
			streamFn: () => {
				requests += 1;
				const response = createAssistantMessageEventStream();
				queueMicrotask(() => {
					response.push({
						type: "done",
						reason: "stop",
						message: assistant([{ type: "text", text: "continued" }]),
					});
				});
				return response;
			},
		});
		agent.followUp({ role: "user", content: "next", timestamp: Date.now() });

		await agent.continue();

		expect(requests).toBe(1);
		expect(agent.checkpointScheduler()?.abi).toBe(COMPILED_AGENT_LOOP_ABI_VERSION);
		expect(agent.state.messages.map((message) => message.role)).toEqual([
			"user",
			"assistant",
			"user",
			"assistant",
		]);
	});
});
