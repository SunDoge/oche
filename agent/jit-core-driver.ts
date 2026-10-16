import { spawn, type ChildProcessWithoutNullStreams } from "node:child_process";
import { createInterface } from "node:readline";
import {
	type ContinueEntryInput,
	type ContinueEntryPlan,
	type ContinuePlan,
	type ContinuePlanInput,
	Effect,
	type MachineState,
	type ModelCompletionPlan,
	type ModelExecutionPlan,
	type OcheAgentCheckpoint,
	type QueueCompletion,
	type QueueDecision,
	type QueueDrainInput,
	type QueueDrainPlan,
	type QueuePollPlan,
	type ToolChangeInput,
	type ToolChangeEntry,
} from "./pi-core-driver.ts";

interface RpcSuccess {
	id: number;
	ok: true;
	value: unknown;
}

interface RpcFailure {
	id: number | null;
	ok: false;
	error: string;
}

type RpcResponse = RpcSuccess | RpcFailure;

interface PendingRequest {
	resolve(value: unknown): void;
	reject(error: Error): void;
}

/** Persistent newline-delimited JSON connection to one Dart VM JIT process. */
export class OcheJitModule {
	readonly #child: ChildProcessWithoutNullStreams;
	readonly #pending = new Map<number, PendingRequest>();
	#nextId = 1;
	#callCount = 0;
	#stderr = "";
	#closed = false;

	constructor(ocheExecutable: string, source: string) {
		this.#child = spawn(ocheExecutable, ["serve", source], { stdio: "pipe" });
		this.#child.stderr.setEncoding("utf8");
		this.#child.stderr.on("data", (chunk: string) => {
			this.#stderr = (this.#stderr + chunk).slice(-16_384);
		});
		createInterface({ input: this.#child.stdout }).on("line", (line) => this.#receive(line));
		this.#child.on("error", (error) => this.#failAll(error));
		this.#child.on("exit", (code, signal) => {
			this.#closed = true;
			if (this.#pending.size === 0) return;
			const detail = this.#stderr.trim();
			this.#failAll(
				new Error(
					`Oche JIT process exited with ${signal ? `signal ${signal}` : `code ${code}`}${detail ? `: ${detail}` : ""}`,
				),
			);
		});
	}

	get pid(): number {
		if (this.#child.pid === undefined) throw new Error("Oche JIT process has no process id");
		return this.#child.pid;
	}

	get callCount(): number {
		return this.#callCount;
	}

	call<T>(method: string, ...args: Array<string | number | boolean>): Promise<T> {
		if (this.#closed || this.#child.stdin.destroyed) {
			return Promise.reject(new Error("Oche JIT process is closed"));
		}
		const id = this.#nextId++;
		this.#callCount += 1;
		return new Promise<T>((resolve, reject) => {
			this.#pending.set(id, {
				resolve: (value) => resolve(value as T),
				reject,
			});
			this.#child.stdin.write(`${JSON.stringify({ id, method, args })}\n`, (error) => {
				if (!error) return;
				this.#pending.delete(id);
				reject(error);
			});
		});
	}

	async close(): Promise<void> {
		if (this.#closed) return;
		const exited = new Promise<void>((resolve) => this.#child.once("exit", () => resolve()));
		this.#child.stdin.end();
		await exited;
	}

	#receive(line: string): void {
		let response: RpcResponse;
		try {
			response = JSON.parse(line) as RpcResponse;
		} catch (error) {
			this.#failAll(new Error(`Oche JIT returned invalid JSON: ${line}`, { cause: error }));
			return;
		}
		if (typeof response.id !== "number") {
			this.#failAll(new Error(`Oche JIT returned a response without a numeric id: ${line}`));
			return;
		}
		const pending = this.#pending.get(response.id);
		if (!pending) {
			this.#failAll(new Error(`Oche JIT returned unknown response id ${response.id}`));
			return;
		}
		this.#pending.delete(response.id);
		if (response.ok) pending.resolve(response.value);
		else pending.reject(new Error(`Oche JIT call failed: ${response.error}`));
	}

	#failAll(error: Error): void {
		for (const request of this.#pending.values()) request.reject(error);
		this.#pending.clear();
	}
}

/** One Dart VM JIT process shared by any number of independently encoded machines. */
export class OcheJitRuntime {
	readonly #module: OcheJitModule;

	private constructor(module: OcheJitModule) {
		this.#module = module;
	}

	static async connect(ocheExecutable: string, source: string): Promise<OcheJitRuntime> {
		const module = new OcheJitModule(ocheExecutable, source);
		try {
			await validateAgentAbi(module);
			return new OcheJitRuntime(module);
		} catch (error) {
			await module.close();
			throw error;
		}
	}

	get pid(): number {
		return this.#module.pid;
	}

	get callCount(): number {
		return this.#module.callCount;
	}

	createMachine(): OcheJitAgentMachine {
		return OcheJitAgentMachine.attach(this.#module);
	}

	close(): Promise<void> {
		return this.#module.close();
	}
}

/** Async agent state machine backed by a single persistent Dart VM JIT process. */
export class OcheJitAgentMachine {
	readonly #module: OcheJitModule;
	readonly #ownsModule: boolean;
	#encoded = "";
	#state: MachineState | undefined;
	#resumeOnEnter = false;
	#preparedRestore: string | undefined;

	private constructor(module: OcheJitModule, ownsModule: boolean) {
		this.#module = module;
		this.#ownsModule = ownsModule;
	}

	static async connect(ocheExecutable: string, source: string): Promise<OcheJitAgentMachine> {
		const module = new OcheJitModule(ocheExecutable, source);
		try {
			await validateAgentAbi(module);
			return new OcheJitAgentMachine(module, true);
		} catch (error) {
			await module.close();
			throw error;
		}
	}

	static attach(module: OcheJitModule): OcheJitAgentMachine {
		return new OcheJitAgentMachine(module, false);
	}

	get pid(): number {
		return this.#module.pid;
	}

	get state(): MachineState {
		if (!this.#state) throw new Error("Agent machine has not started");
		return this.#state;
	}

	get started(): boolean {
		return this.#state !== undefined;
	}

	async start(): Promise<MachineState> {
		this.#resumeOnEnter = false;
		this.#preparedRestore = undefined;
		return this.#accept(await this.#module.call<string>("start"));
	}

	async enterLoop(): Promise<MachineState> {
		if (!this.#resumeOnEnter) return this.start();
		this.#resumeOnEnter = false;
		return this.#accept(await this.#module.call<string>("enter", this.#encoded));
	}

	async beginLoop(messages: readonly unknown[]): Promise<MachineState> {
		const last = summarizeMessage(messages.at(-1));
		if (this.#preparedRestore !== undefined) {
			const source = this.#preparedRestore;
			const state = this.#accept(
				await this.#module.call<string>(
					"restore_begin_with_messages",
					source,
					messages.length,
					last.role,
					last.toolCalls,
					last.failed,
				),
			);
			this.#preparedRestore = undefined;
			this.#resumeOnEnter = false;
			return state;
		}
		const resume = this.#resumeOnEnter;
		this.#resumeOnEnter = false;
		return this.#accept(
			await this.#module.call<string>(
				"begin_with_messages",
				this.#encoded,
				resume,
				messages.length,
				last.role,
				last.toolCalls,
				last.failed,
			),
		);
	}

	async beginLoopWithToolChanges(input: ToolChangeInput): Promise<ToolChangeEntry> {
		const keep = summarizeMessages(input.keepMessages);
		const rewrite = summarizeMessages(input.rewriteMessages);
		const insert = summarizeMessages(input.insertMessages);
		const prepared = this.#preparedRestore;
		const resume = prepared !== undefined || this.#resumeOnEnter;
		const encoded = await this.#module.call<string>(
			"begin_with_tool_changes",
			prepared ?? this.#encoded,
			resume,
			input.changed,
			input.hasPending,
			input.pendingHadChanges,
			keep.count,
			keep.last.role,
			keep.last.toolCalls,
			keep.last.failed,
			rewrite.count,
			rewrite.last.role,
			rewrite.last.toolCalls,
			rewrite.last.failed,
			insert.count,
			insert.last.role,
			insert.last.toolCalls,
			insert.last.failed,
		);
		const plan = JSON.parse(encoded) as { action: number; state: string };
		const state = this.#accept(plan.state);
		this.#preparedRestore = undefined;
		this.#resumeOnEnter = false;
		return { action: plan.action, state };
	}

	async beginContinueWithToolChanges(input: ContinueEntryInput): Promise<ContinueEntryPlan> {
		const roles = input.messages.map((message) => messageRole(message));
		if (input.steeringToolChanges && input.followUpToolChanges) {
			const prepared = this.#preparedRestore;
			const resume = prepared !== undefined || this.#resumeOnEnter;
			try {
				const encoded = await this.#module.call<string>(
					"begin_continuation_with_tool_changes",
					prepared ?? this.#encoded,
					resume,
					roles.length,
					roles.filter((role) => role === "system").length,
					roles.at(-1) ?? "",
					input.steering,
					input.followUp,
					input.steeringTakeAll,
					input.followUpTakeAll,
					JSON.stringify({
						direct: summarizeToolChanges(input.toolChanges),
						steering: summarizeToolChanges(input.steeringToolChanges),
						follow_up: summarizeToolChanges(input.followUpToolChanges),
					}),
				);
				return this.#acceptContinuationEntry(encoded);
			} catch (error) {
				if (!(error instanceof Error) || !error.message.includes("unknown Oche function")) throw error;
			}
		}
		const keep = summarizeMessages(input.toolChanges.keepMessages);
		const rewrite = summarizeMessages(input.toolChanges.rewriteMessages);
		const insert = summarizeMessages(input.toolChanges.insertMessages);
		const prepared = this.#preparedRestore;
		const resume = prepared !== undefined || this.#resumeOnEnter;
		const encoded = await this.#module.call<string>(
			"begin_continue_with_tool_changes",
			prepared ?? this.#encoded,
			resume,
			roles.length,
			roles.filter((role) => role === "system").length,
			roles.at(-1) ?? "",
			input.steering,
			input.followUp,
			input.steeringTakeAll,
			input.followUpTakeAll,
			input.toolChanges.changed,
			input.toolChanges.hasPending,
			input.toolChanges.pendingHadChanges,
			keep.count,
			keep.last.role,
			keep.last.toolCalls,
			keep.last.failed,
			rewrite.count,
			rewrite.last.role,
			rewrite.last.toolCalls,
			rewrite.last.failed,
			insert.count,
			insert.last.role,
			insert.last.toolCalls,
			insert.last.failed,
		);
		return this.#acceptContinuationEntry(encoded);
	}

	#acceptContinuationEntry(encoded: string): ContinueEntryPlan {
		const raw = JSON.parse(encoded) as {
			action: number;
			queue: number;
			take: number;
			tool_action: number;
			state: string;
		};
		const plan: ContinueEntryPlan = { action: raw.action, queue: raw.queue, take: raw.take };
		if (raw.action !== 1 && raw.action !== 2 && raw.action !== 3) return plan;
		if (raw.state === "") return plan;
		const state = this.#accept(raw.state);
		this.#preparedRestore = undefined;
		this.#resumeOnEnter = false;
		return { ...plan, toolAction: raw.tool_action, state };
	}

	checkpoint(): OcheAgentCheckpoint {
		if (!this.#state) throw new Error("Agent machine has not started");
		return { format: 2, state: this.#encoded };
	}

	checkpointScheduler(): { abi: 2; state: string } | undefined {
		if (this.#preparedRestore !== undefined) return { abi: 2, state: this.#preparedRestore };
		return this.#state ? { abi: 2, state: this.#encoded } : undefined;
	}

	prepareSchedulerRestore(checkpoint: { abi: number; state: string }): void {
		if (checkpoint.abi !== 2) {
			throw new Error(`Unsupported Oche scheduler checkpoint ABI: ${checkpoint.abi}`);
		}
		this.#preparedRestore = checkpoint.state;
	}

	async restore(checkpoint: OcheAgentCheckpoint): Promise<MachineState> {
		this.#preparedRestore = undefined;
		if (checkpoint.format !== 2) {
			throw new Error(`Unsupported Oche JIT checkpoint format: ${checkpoint.format}`);
		}
		const state = this.#accept(await this.#module.call<string>("restore", checkpoint.state));
		this.#resumeOnEnter = true;
		return state;
	}

	async restoreScheduler(checkpoint: { abi: number; state: string }): Promise<MachineState> {
		if (checkpoint.abi !== 2) {
			throw new Error(`Unsupported Oche scheduler checkpoint ABI: ${checkpoint.abi}`);
		}
		return this.restore({ format: 2, state: checkpoint.state });
	}

	async observeMessage(role: string, toolCalls = 0, failed = false): Promise<MachineState> {
		return this.#accept(
			await this.#module.call<string>("observe_message", this.#encoded, role, toolCalls, failed),
		);
	}

	async observeMessages(messages: readonly unknown[]): Promise<void> {
		if (messages.length === 0) return;
		const last = summarizeMessage(messages.at(-1));
		this.#accept(
			await this.#module.call<string>(
				"observe_message_batch",
				this.#encoded,
				messages.length,
				last.role,
				last.toolCalls,
				last.failed,
			),
		);
	}

	async observeToolBatch(summary: { total: number; failed: number; terminate: boolean }): Promise<void> {
		this.#accept(
			await this.#module.call<string>(
				"observe_tool_batch",
				this.#encoded,
				summary.total,
				summary.failed,
				summary.terminate,
			),
		);
	}

	async observeQueues(decision: {
		steeringMessages?: number;
		followUpMessages?: number;
		forceContinue?: boolean;
		forceEnd?: boolean;
	}): Promise<void> {
		this.#accept(
			await this.#module.call<string>(
				"observe_queues",
				this.#encoded,
				decision.steeringMessages ?? 0,
				decision.followUpMessages ?? 0,
				decision.forceContinue ?? false,
				decision.forceEnd ?? false,
			),
		);
	}

	async modelCompleted(toolCalls: number, failed = false): Promise<MachineState> {
		return this.#accept(
			await this.#module.call<string>("model_completed", this.#encoded, toolCalls, failed),
		);
	}

	async completeModel(message: unknown): Promise<MachineState> {
		const summary = summarizeMessage(message);
		return this.#accept(
			await this.#module.call<string>(
				"complete_model_turn",
				this.#encoded,
				summary.role,
				summary.toolCalls,
				summary.failed,
			),
		);
	}

	async completeModelPlan(
		message: unknown,
		globalSequential: boolean,
		hasSequential: boolean,
	): Promise<ModelCompletionPlan> {
		const summary = summarizeMessage(message);
		const encoded = await this.#module.call<string>(
			"complete_model_plan",
			this.#encoded,
			summary.role,
			summary.toolCalls,
			summary.failed,
			globalSequential,
			hasSequential,
		);
		const plan = JSON.parse(encoded) as { state: string; run_tools_sequential: boolean };
		return {
			state: this.#accept(plan.state),
			runToolsSequential: plan.run_tools_sequential,
		};
	}

	async completeModelExecutionPlan(
		message: unknown,
		globalSequential: boolean,
		hasSequential: boolean,
	): Promise<ModelExecutionPlan> {
		const summary = summarizeMessage(message);
		const encoded = await this.#module.call<string>(
			"complete_model_execution_plan",
			this.#encoded,
			summary.role,
			summary.toolCalls,
			summary.failed,
			summary.role === "assistant" && messageStopReason(message) === "length",
			globalSequential,
			hasSequential,
		);
		const plan = JSON.parse(encoded) as { state: string; tool_action: number };
		return { state: this.#accept(plan.state), toolAction: plan.tool_action };
	}

	async toolsCompleted(terminate = false): Promise<MachineState> {
		return this.#accept(await this.#module.call<string>("tools_completed", this.#encoded, terminate));
	}

	async completeTools(
		messages: readonly unknown[],
		summary: { total: number; failed: number; terminate: boolean },
	): Promise<MachineState> {
		const last = summarizeMessage(messages.at(-1));
		return this.#accept(
			await this.#module.call<string>(
				"complete_tool_turn",
				this.#encoded,
				messages.length,
				summary.total,
				summary.failed,
				summary.terminate,
				last.role,
				last.failed,
			),
		);
	}

	async completeToolOutcomes(
		messages: readonly unknown[],
		summary: { total: number; failed: number; terminating: number },
	): Promise<MachineState> {
		const last = summarizeMessage(messages.at(-1));
		return this.#accept(
			await this.#module.call<string>(
				"complete_tool_outcomes",
				this.#encoded,
				messages.length,
				summary.total,
				summary.failed,
				summary.terminating,
				last.role,
				last.failed,
			),
		);
	}

	async queuesCompleted(
		steeringMessages = 0,
		followUpMessages = 0,
		forceContinue = false,
		forceEnd = false,
	): Promise<MachineState> {
		return this.#accept(
			await this.#module.call<string>(
				"queues_completed",
				this.#encoded,
				steeringMessages,
				followUpMessages,
				forceContinue,
				forceEnd,
			),
		);
	}

	async completeQueues(decision: QueueDecision): Promise<QueueCompletion> {
		const encoded = await this.#module.call<string>(
			"complete_queue_plan",
			this.#encoded,
			decision.steeringMessages ?? 0,
			decision.followUpMessages ?? 0,
			decision.forceContinue ?? false,
			decision.forceEnd ?? false,
		);
		const completion = JSON.parse(encoded) as { queue: number; state: string };
		return { queue: completion.queue, state: this.#accept(completion.state) };
	}

	async planQueuePoll(decision: QueueDecision): Promise<QueuePollPlan> {
		const encoded = await this.#module.call<string>(
			"queue_poll_plan",
			this.#encoded,
			decision.steeringMessages ?? 0,
			decision.forceContinue ?? false,
			decision.forceEnd ?? false,
		);
		const plan = JSON.parse(encoded) as { poll_follow_up: boolean; completion: string };
		if (plan.poll_follow_up) return { pollFollowUp: true };
		const completion = JSON.parse(plan.completion) as { queue: number; state: string };
		return {
			pollFollowUp: false,
			completion: { queue: completion.queue, state: this.#accept(completion.state) },
		};
	}

	async planQueueDrain(input: QueueDrainInput): Promise<QueueDrainPlan> {
		const encoded = await this.#module.call<string>(
			"complete_queue_drain_plan",
			this.#encoded,
			input.steering,
			input.followUp,
			input.steeringTakeAll,
			input.followUpTakeAll,
			input.forceContinue,
			input.forceEnd,
		);
		const plan = JSON.parse(encoded) as { queue: number; take: number; state: string };
		return { queue: plan.queue, take: plan.take, state: this.#accept(plan.state) };
	}

	advanceModel(): Promise<MachineState> {
		return this.modelCompleted(0, false);
	}

	advanceTools(): Promise<MachineState> {
		return this.toolsCompleted(false);
	}

	advanceQueues(): Promise<MachineState> {
		return this.queuesCompleted();
	}

	async classifyMessageTail(messages: readonly unknown[]): Promise<number> {
		const roles = messages.map((message) => messageRole(message));
		return this.#module.call<number>(
			"classify_message_tail",
			roles.length,
			roles.filter((role) => role === "system").length,
			roles.at(-1) ?? "",
		);
	}

	toolBatchTerminates(total: number, terminating: number): Promise<boolean> {
		return this.#module.call<boolean>("tool_batch_terminates", total, terminating);
	}

	queueTake(available: number, takeAll: boolean): Promise<number> {
		return this.#module.call<number>("queue_take", available, takeAll);
	}

	toolsRunSequential(globalSequential: boolean, hasSequential: boolean): Promise<boolean> {
		return this.#module.call<boolean>("tools_run_sequential", globalSequential, hasSequential);
	}

	selectQueue(steering: number, followUp: number): Promise<number> {
		return this.#module.call<number>("select_queue", steering, followUp);
	}

	continueAction(messageKind: number, queue: number): Promise<number> {
		return this.#module.call<number>("continue_action", messageKind, queue);
	}

	async planContinue(input: ContinuePlanInput): Promise<ContinuePlan> {
		const roles = input.messages.map((message) => messageRole(message));
		const encoded = await this.#module.call<string>(
			"continue_plan",
			roles.length,
			roles.filter((role) => role === "system").length,
			roles.at(-1) ?? "",
			input.steering,
			input.followUp,
			input.steeringTakeAll,
			input.followUpTakeAll,
		);
		return JSON.parse(encoded) as ContinuePlan;
	}

	pollFollowUp(steering: number, forceEnd: boolean): Promise<boolean> {
		return this.#module.call<boolean>("should_poll_follow_up", this.state.must_continue, steering, forceEnd);
	}

	toolChangeAction(changed: boolean, hasPending: boolean, pendingHadChanges: boolean): Promise<number> {
		return this.#module.call<number>("tool_change_action", changed, hasPending, pendingHadChanges);
	}

	async completeToolChanges(input: ToolChangeInput): Promise<number> {
		if (isEmptyToolChangeInput(input)) return 0;
		const keep = summarizeMessages(input.keepMessages);
		const rewrite = summarizeMessages(input.rewriteMessages);
		const insert = summarizeMessages(input.insertMessages);
		const encoded = await this.#module.call<string>(
			"complete_tool_changes",
			this.#encoded,
			input.changed,
			input.hasPending,
			input.pendingHadChanges,
			keep.count,
			keep.last.role,
			keep.last.toolCalls,
			keep.last.failed,
			rewrite.count,
			rewrite.last.role,
			rewrite.last.toolCalls,
			rewrite.last.failed,
			insert.count,
			insert.last.role,
			insert.last.toolCalls,
			insert.last.failed,
		);
		const plan = JSON.parse(encoded) as { action: number; state: string };
		this.#accept(plan.state);
		return plan.action;
	}

	call<T>(method: string, ...args: Array<string | number | boolean>): Promise<T> {
		return this.#module.call<T>(method, ...args);
	}

	close(): Promise<void> {
		return this.#ownsModule ? this.#module.close() : Promise.resolve();
	}

	#accept(encoded: string): MachineState {
		const state = JSON.parse(encoded) as MachineState;
		if (state.effect === Effect.Invalid) throw new Error(`Invalid Oche machine state: ${state.error}`);
		this.#encoded = encoded;
		this.#state = state;
		return state;
	}
}

async function validateAgentAbi(module: OcheJitModule): Promise<void> {
	const version = await module.call<number>("abi_version");
	if (version !== 2) throw new Error(`Unsupported Oche agent ABI: ${version}; expected 2`);
}

function messageRole(message: unknown): string {
	return typeof message === "object" && message !== null && "role" in message && typeof message.role === "string"
		? message.role
		: "custom";
}

function messageStopReason(message: unknown): string | undefined {
	return typeof message === "object" &&
		message !== null &&
		"stopReason" in message &&
		typeof message.stopReason === "string"
		? message.stopReason
		: undefined;
}

function summarizeMessage(message: unknown): { role: string; toolCalls: number; failed: boolean } {
	if (typeof message !== "object" || message === null) {
		return { role: "custom", toolCalls: 0, failed: false };
	}
	const role = messageRole(message);
	const content = "content" in message && Array.isArray(message.content) ? message.content : [];
	const toolCalls = content.filter(
		(item) => typeof item === "object" && item !== null && "type" in item && item.type === "toolCall",
	).length;
	const failed =
		("stopReason" in message && (message.stopReason === "error" || message.stopReason === "aborted")) ||
		("isError" in message && message.isError === true);
	return { role, toolCalls, failed };
}

function summarizeMessages(messages: readonly unknown[]): {
	count: number;
	last: { role: string; toolCalls: number; failed: boolean };
} {
	return {
		count: messages.length,
		last: summarizeMessage(messages.at(-1)),
	};
}

function summarizeToolChanges(input: ToolChangeInput): unknown {
	return {
		changed: input.changed,
		has_pending: input.hasPending,
		pending_had_changes: input.pendingHadChanges,
		keep: summarizeMessageBatch(input.keepMessages),
		rewrite: summarizeMessageBatch(input.rewriteMessages),
		insert: summarizeMessageBatch(input.insertMessages),
	};
}

function isEmptyToolChangeInput(input: ToolChangeInput): boolean {
	return (
		!input.changed &&
		!input.hasPending &&
		!input.pendingHadChanges &&
		input.keepMessages.length === 0
	);
}

function summarizeMessageBatch(messages: readonly unknown[]): unknown {
	const summary = summarizeMessages(messages);
	return {
		count: summary.count,
		last_role: summary.last.role,
		last_tool_calls: summary.last.toolCalls,
		last_failed: summary.last.failed,
	};
}
