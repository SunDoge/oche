export const Effect = {
	RequestModel: 1,
	ExecuteTools: 2,
	PollQueues: 3,
	Done: 4,
	Invalid: 5,
} as const;

export type Effect = (typeof Effect)[keyof typeof Effect];

export interface MachineState {
	schema: 2;
	phase: number;
	turn: number;
	must_continue: boolean;
	effect: Effect;
	error: string;
	transition_count: number;
	message_count: number;
	history: string[];
	messages: MessageSummary[];
	tool_batch: ToolBatchSummary;
	queues: QueueSummary;
}

export interface MessageSummary {
	role: string;
	tool_calls: number;
	failed: boolean;
}

export interface ToolBatchSummary {
	observed: boolean;
	total: number;
	failed: number;
	terminate: boolean;
}

export interface QueueSummary {
	observed: boolean;
	steering: number;
	follow_up: number;
	force_continue: boolean;
	force_end: boolean;
}

export interface ModelTurn<TModelTurn> {
	value: TModelTurn;
	toolCalls: number;
	failed?: boolean;
}

export interface QueueDecision {
	steeringMessages?: number;
	followUpMessages?: number;
	forceContinue?: boolean;
	forceEnd?: boolean;
}

export interface OcheAgentHost<TModelTurn> {
	requestModel(state: MachineState, signal?: AbortSignal): Promise<ModelTurn<TModelTurn>>;
	executeTools(
		turn: TModelTurn,
		state: MachineState,
		signal?: AbortSignal,
	): Promise<{ total?: number; failed?: number; terminate?: boolean }>;
	pollQueues(state: MachineState, signal?: AbortSignal): Promise<QueueDecision>;
}

export interface OcheAgentRun {
	reason: "done" | "aborted";
	state: MachineState;
}

export interface OcheAgentCheckpoint {
	format: 1 | 2;
	state: string;
}

export interface ContinuePlanInput {
	messages: readonly unknown[];
	steering: number;
	followUp: number;
	steeringTakeAll: boolean;
	followUpTakeAll: boolean;
}

export interface ContinuePlan {
	action: number;
	queue: number;
	take: number;
}

export interface ContinueEntryInput extends ContinuePlanInput {
	toolChanges: ToolChangeInput;
	steeringToolChanges?: ToolChangeInput;
	followUpToolChanges?: ToolChangeInput;
}

export interface ContinueEntryPlan extends ContinuePlan {
	toolAction?: number;
	state?: MachineState;
}

export interface QueueCompletion {
	queue: number;
	state: MachineState;
}

export interface QueuePollPlan {
	pollFollowUp: boolean;
	completion?: QueueCompletion;
}

export interface QueueDrainInput {
	steering: number;
	followUp: number;
	steeringTakeAll: boolean;
	followUpTakeAll: boolean;
	forceContinue: boolean;
	forceEnd: boolean;
}

export interface QueueDrainPlan {
	queue: number;
	take: number;
	state: MachineState;
}

export interface ModelCompletionPlan {
	state: MachineState;
	runToolsSequential: boolean;
}

export interface ModelExecutionPlan {
	state: MachineState;
	toolAction: number;
}

export interface ToolChangeInput {
	changed: boolean;
	hasPending: boolean;
	pendingHadChanges: boolean;
	keepMessages: readonly unknown[];
	rewriteMessages: readonly unknown[];
	insertMessages: readonly unknown[];
}

export interface ToolChangeEntry {
	action: number;
	state: MachineState;
}

interface OchePiCore {
	abi_version(): number;
	start(): string;
	enter(state: string): string;
	restore(state: string): string;
	restore_begin_with_messages(
		state: string,
		count: number,
		lastRole: string,
		lastToolCalls: number,
		lastFailed: boolean,
	): string;
	observe_message(state: string, role: string, toolCalls: number, failed: boolean): string;
	observe_message_batch(
		state: string,
		count: number,
		lastRole: string,
		lastToolCalls: number,
		lastFailed: boolean,
	): string;
	begin_with_messages(
		state: string,
		resume: boolean,
		count: number,
		lastRole: string,
		lastToolCalls: number,
		lastFailed: boolean,
	): string;
	begin_with_tool_changes(
		state: string,
		resume: boolean,
		changed: boolean,
		hasPending: boolean,
		pendingHadChanges: boolean,
		keepCount: number,
		keepLastRole: string,
		keepLastToolCalls: number,
		keepLastFailed: boolean,
		rewriteCount: number,
		rewriteLastRole: string,
		rewriteLastToolCalls: number,
		rewriteLastFailed: boolean,
		insertCount: number,
		insertLastRole: string,
		insertLastToolCalls: number,
		insertLastFailed: boolean,
	): string;
	begin_continue_with_tool_changes(
		state: string,
		resume: boolean,
		total: number,
		system: number,
		tailRole: string,
		steering: number,
		followUp: number,
		steeringTakeAll: boolean,
		followUpTakeAll: boolean,
		changed: boolean,
		hasPending: boolean,
		pendingHadChanges: boolean,
		keepCount: number,
		keepLastRole: string,
		keepLastToolCalls: number,
		keepLastFailed: boolean,
		rewriteCount: number,
		rewriteLastRole: string,
		rewriteLastToolCalls: number,
		rewriteLastFailed: boolean,
		insertCount: number,
		insertLastRole: string,
		insertLastToolCalls: number,
		insertLastFailed: boolean,
	): string;
	begin_continuation_with_tool_changes?(
		state: string,
		resume: boolean,
		total: number,
		system: number,
		tailRole: string,
		steering: number,
		followUp: number,
		steeringTakeAll: boolean,
		followUpTakeAll: boolean,
		candidates: string,
	): string;
	observe_tool_batch(state: string, total: number, failed: number, terminate: boolean): string;
	observe_queues(
		state: string,
		steering: number,
		followUp: number,
		forceContinue: boolean,
		forceEnd: boolean,
	): string;
	observed_message_count(state: string): number;
	tail_message_kind(state: string): number;
	classify_message_tail(total: number, system: number, tailRole: string): number;
	tool_batch_terminates(total: number, terminating: number): boolean;
	queue_take(available: number, takeAll: boolean): number;
	tools_run_sequential(globalSequential: boolean, hasSequential: boolean): boolean;
	select_queue(steering: number, followUp: number): number;
	continue_action(messageKind: number, queue: number): number;
	continue_plan(
		total: number,
		system: number,
		tailRole: string,
		steering: number,
		followUp: number,
		steeringTakeAll: boolean,
		followUpTakeAll: boolean,
	): string;
	should_poll_follow_up(naturalContinue: boolean, steering: number, forceEnd: boolean): boolean;
	tool_change_action(changed: boolean, hasPending: boolean, pendingHadChanges: boolean): number;
	complete_tool_changes(
		state: string,
		changed: boolean,
		hasPending: boolean,
		pendingHadChanges: boolean,
		keepCount: number,
		keepLastRole: string,
		keepLastToolCalls: number,
		keepLastFailed: boolean,
		rewriteCount: number,
		rewriteLastRole: string,
		rewriteLastToolCalls: number,
		rewriteLastFailed: boolean,
		insertCount: number,
		insertLastRole: string,
		insertLastToolCalls: number,
		insertLastFailed: boolean,
	): string;
	model_completed(state: string, toolCalls: number, failed: boolean): string;
	complete_model_turn(state: string, role: string, toolCalls: number, failed: boolean): string;
	complete_model_plan(
		state: string,
		role: string,
		toolCalls: number,
		failed: boolean,
		globalSequential: boolean,
		hasSequential: boolean,
	): string;
	complete_model_execution_plan(
		state: string,
		role: string,
		toolCalls: number,
		failed: boolean,
		truncated: boolean,
		globalSequential: boolean,
		hasSequential: boolean,
	): string;
	tools_completed(state: string, terminate: boolean): string;
	complete_tool_turn(
		state: string,
		messageCount: number,
		total: number,
		failed: number,
		terminate: boolean,
		lastRole: string,
		lastFailed: boolean,
	): string;
	complete_tool_outcomes(
		state: string,
		messageCount: number,
		total: number,
		failed: number,
		terminating: number,
		lastRole: string,
		lastFailed: boolean,
	): string;
	queues_completed(
		state: string,
		steeringMessages: number,
		followUpMessages: number,
		forceContinue: boolean,
		forceEnd: boolean,
	): string;
	complete_queue_plan(
		state: string,
		steeringMessages: number,
		followUpMessages: number,
		forceContinue: boolean,
		forceEnd: boolean,
	): string;
	queue_poll_plan(
		state: string,
		steeringMessages: number,
		forceContinue: boolean,
		forceEnd: boolean,
	): string;
	complete_queue_drain_plan(
		state: string,
		steering: number,
		followUp: number,
		steeringTakeAll: boolean,
		followUpTakeAll: boolean,
		forceContinue: boolean,
		forceEnd: boolean,
	): string;
}

export class OcheAgentMachine {
	readonly #core: OchePiCore;
	#encoded = "";
	#state: MachineState | undefined;
	#resumeOnEnter = false;

	constructor(core: OchePiCore) {
		const version = core.abi_version();
		if (version !== 2) throw new Error(`Unsupported Oche agent ABI: ${version}; expected 2`);
		this.#core = core;
	}

	get state(): MachineState {
		if (!this.#state) throw new Error("Agent machine has not started");
		return this.#state;
	}

	get started(): boolean {
		return this.#state !== undefined;
	}

	start(): MachineState {
		this.#resumeOnEnter = false;
		return this.#accept(this.#core.start());
	}

	enterLoop(): MachineState {
		if (this.#resumeOnEnter) {
			this.#resumeOnEnter = false;
			return this.#accept(this.#core.enter(this.#encoded));
		}
		return this.start();
	}

	beginLoop(messages: readonly unknown[]): MachineState {
		const last = summarizeMessage(messages.at(-1));
		const resume = this.#resumeOnEnter;
		this.#resumeOnEnter = false;
		return this.#accept(
			this.#core.begin_with_messages(
				this.#encoded,
				resume,
				messages.length,
				last.role,
				last.tool_calls,
				last.failed,
			),
		);
	}

	beginLoopWithToolChanges(input: ToolChangeInput): ToolChangeEntry {
		const keep = summarizeMessages(input.keepMessages);
		const rewrite = summarizeMessages(input.rewriteMessages);
		const insert = summarizeMessages(input.insertMessages);
		const resume = this.#resumeOnEnter;
		const plan = JSON.parse(
			this.#core.begin_with_tool_changes(
				this.#encoded,
				resume,
				input.changed,
				input.hasPending,
				input.pendingHadChanges,
				keep.count,
				keep.last.role,
				keep.last.tool_calls,
				keep.last.failed,
				rewrite.count,
				rewrite.last.role,
				rewrite.last.tool_calls,
				rewrite.last.failed,
				insert.count,
				insert.last.role,
				insert.last.tool_calls,
				insert.last.failed,
			),
		) as { action: number; state: string };
		const state = this.#accept(plan.state);
		this.#resumeOnEnter = false;
		return { action: plan.action, state };
	}

	beginContinueWithToolChanges(input: ContinueEntryInput): ContinueEntryPlan {
		const roles = input.messages.map((message) => messageRole(message));
		if (
			this.#core.begin_continuation_with_tool_changes &&
			input.steeringToolChanges &&
			input.followUpToolChanges
		) {
			return this.#acceptContinuationEntry(
				this.#core.begin_continuation_with_tool_changes(
					this.#encoded,
					this.#resumeOnEnter,
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
				),
			);
		}
		const keep = summarizeMessages(input.toolChanges.keepMessages);
		const rewrite = summarizeMessages(input.toolChanges.rewriteMessages);
		const insert = summarizeMessages(input.toolChanges.insertMessages);
		const encoded = this.#core.begin_continue_with_tool_changes(
			this.#encoded,
			this.#resumeOnEnter,
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
			keep.last.tool_calls,
			keep.last.failed,
			rewrite.count,
			rewrite.last.role,
			rewrite.last.tool_calls,
			rewrite.last.failed,
			insert.count,
			insert.last.role,
			insert.last.tool_calls,
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
		this.#resumeOnEnter = false;
		return { ...plan, toolAction: raw.tool_action, state };
	}

	restoreBegin(checkpoint: OcheAgentCheckpoint, messages: readonly unknown[]): MachineState {
		if (checkpoint.format !== 2) {
			throw new Error(`Atomic Oche restore requires checkpoint format 2; received ${checkpoint.format}`);
		}
		const last = summarizeMessage(messages.at(-1));
		this.#resumeOnEnter = false;
		return this.#accept(
			this.#core.restore_begin_with_messages(
				checkpoint.state,
				messages.length,
				last.role,
				last.tool_calls,
				last.failed,
			),
		);
	}

	checkpoint(): OcheAgentCheckpoint {
		if (!this.#state) throw new Error("Agent machine has not started");
		return { format: 2, state: this.#encoded };
	}

	restore(checkpoint: OcheAgentCheckpoint): MachineState {
		if (checkpoint.format === 2) {
			const state = this.#accept(this.#core.restore(checkpoint.state));
			this.#resumeOnEnter = true;
			return state;
		}
		if (checkpoint.format === 1) {
			const state = this.#accept(this.#core.restore(migrateCheckpointV1(checkpoint.state)));
			this.#resumeOnEnter = true;
			return state;
		}
		throw new Error(`Unsupported Oche agent checkpoint format: ${checkpoint.format}`);
	}

	observeMessages(messages: readonly unknown[]): void {
		if (messages.length === 0) return;
		const last = summarizeMessage(messages.at(-1));
		this.#accept(
			this.#core.observe_message_batch(
				this.#encoded,
				messages.length,
				last.role,
				last.tool_calls,
				last.failed,
			),
		);
	}

	observeModel(summary: { toolCalls: number; failed?: boolean }): void {
		this.#accept(
			this.#core.observe_message(
				this.#encoded,
				"assistant",
				summary.toolCalls,
				summary.failed ?? false,
			),
		);
	}

	observeToolBatch(summary: { total: number; failed: number; terminate: boolean }): void {
		this.#accept(
			this.#core.observe_tool_batch(
				this.#encoded,
				summary.total,
				summary.failed,
				summary.terminate,
			),
		);
	}

	observeQueues(decision: {
		steeringMessages?: number;
		followUpMessages?: number;
		forceContinue?: boolean;
		forceEnd?: boolean;
	}): void {
		this.#accept(
			this.#core.observe_queues(
				this.#encoded,
				decision.steeringMessages ?? 0,
				decision.followUpMessages ?? 0,
				decision.forceContinue ?? false,
				decision.forceEnd ?? false,
			),
		);
	}

	completeQueues(decision: QueueDecision): QueueCompletion {
		const completion = JSON.parse(
			this.#core.complete_queue_plan(
				this.#encoded,
				decision.steeringMessages ?? 0,
				decision.followUpMessages ?? 0,
				decision.forceContinue ?? false,
				decision.forceEnd ?? false,
			),
		) as { queue: number; state: string };
		return { queue: completion.queue, state: this.#accept(completion.state) };
	}

	planQueuePoll(decision: QueueDecision): QueuePollPlan {
		const plan = JSON.parse(
			this.#core.queue_poll_plan(
				this.#encoded,
				decision.steeringMessages ?? 0,
				decision.forceContinue ?? false,
				decision.forceEnd ?? false,
			),
		) as { poll_follow_up: boolean; completion: string };
		if (plan.poll_follow_up) return { pollFollowUp: true };
		const completion = JSON.parse(plan.completion) as { queue: number; state: string };
		return {
			pollFollowUp: false,
			completion: { queue: completion.queue, state: this.#accept(completion.state) },
		};
	}

	planQueueDrain(input: QueueDrainInput): QueueDrainPlan {
		const plan = JSON.parse(
			this.#core.complete_queue_drain_plan(
				this.#encoded,
				input.steering,
				input.followUp,
				input.steeringTakeAll,
				input.followUpTakeAll,
				input.forceContinue,
				input.forceEnd,
			),
		) as { queue: number; take: number; state: string };
		return { queue: plan.queue, take: plan.take, state: this.#accept(plan.state) };
	}

	observedMessageCount(): number {
		return this.#core.observed_message_count(this.#encoded);
	}

	tailMessageKind(): number {
		return this.#core.tail_message_kind(this.#encoded);
	}

	classifyMessageTail(messages: readonly unknown[]): number {
		const roles = messages.map((message) =>
			typeof message === "object" && message !== null && "role" in message && typeof message.role === "string"
				? message.role
				: "custom",
		);
		return this.#core.classify_message_tail(
			roles.length,
			roles.filter((role) => role === "system").length,
			roles.at(-1) ?? "",
		);
	}

	toolBatchTerminates(total: number, terminating: number): boolean {
		return this.#core.tool_batch_terminates(total, terminating);
	}

	queueTake(available: number, takeAll: boolean): number {
		return this.#core.queue_take(available, takeAll);
	}

	toolsRunSequential(globalSequential: boolean, hasSequential: boolean): boolean {
		return this.#core.tools_run_sequential(globalSequential, hasSequential);
	}

	selectQueue(steering: number, followUp: number): number {
		return this.#core.select_queue(steering, followUp);
	}

	continueAction(messageKind: number, queue: number): number {
		return this.#core.continue_action(messageKind, queue);
	}

	planContinue(input: ContinuePlanInput): ContinuePlan {
		const roles = input.messages.map((message) => messageRole(message));
		return JSON.parse(
			this.#core.continue_plan(
				roles.length,
				roles.filter((role) => role === "system").length,
				roles.at(-1) ?? "",
				input.steering,
				input.followUp,
				input.steeringTakeAll,
				input.followUpTakeAll,
			),
		) as ContinuePlan;
	}

	pollFollowUp(steering: number, forceEnd: boolean): boolean {
		return this.#core.should_poll_follow_up(this.state.must_continue, steering, forceEnd);
	}

	toolChangeAction(changed: boolean, hasPending: boolean, pendingHadChanges: boolean): number {
		return this.#core.tool_change_action(changed, hasPending, pendingHadChanges);
	}

	completeToolChanges(input: ToolChangeInput): number {
		if (isEmptyToolChangeInput(input)) return 0;
		const keep = summarizeMessages(input.keepMessages);
		const rewrite = summarizeMessages(input.rewriteMessages);
		const insert = summarizeMessages(input.insertMessages);
		const plan = JSON.parse(
			this.#core.complete_tool_changes(
				this.#encoded,
				input.changed,
				input.hasPending,
				input.pendingHadChanges,
				keep.count,
				keep.last.role,
				keep.last.tool_calls,
				keep.last.failed,
				rewrite.count,
				rewrite.last.role,
				rewrite.last.tool_calls,
				rewrite.last.failed,
				insert.count,
				insert.last.role,
				insert.last.tool_calls,
				insert.last.failed,
			),
		) as { action: number; state: string };
		this.#accept(plan.state);
		return plan.action;
	}

	advanceModel(): MachineState {
		return this.modelCompleted(0, false);
	}

	advanceTools(): MachineState {
		return this.toolsCompleted(false);
	}

	advanceQueues(): MachineState {
		return this.queuesCompleted();
	}

	modelCompleted(toolCalls: number, failed = false): MachineState {
		return this.#accept(this.#core.model_completed(this.#encoded, toolCalls, failed));
	}

	completeModel(message: unknown): MachineState {
		const summary = summarizeMessage(message);
		return this.#accept(
			this.#core.complete_model_turn(
				this.#encoded,
				summary.role,
				summary.tool_calls,
				summary.failed,
			),
		);
	}

	completeModelPlan(
		message: unknown,
		globalSequential: boolean,
		hasSequential: boolean,
	): ModelCompletionPlan {
		const summary = summarizeMessage(message);
		const plan = JSON.parse(
			this.#core.complete_model_plan(
				this.#encoded,
				summary.role,
				summary.tool_calls,
				summary.failed,
				globalSequential,
				hasSequential,
			),
		) as { state: string; run_tools_sequential: boolean };
		return {
			state: this.#accept(plan.state),
			runToolsSequential: plan.run_tools_sequential,
		};
	}

	completeModelExecutionPlan(
		message: unknown,
		globalSequential: boolean,
		hasSequential: boolean,
	): ModelExecutionPlan {
		const summary = summarizeMessage(message);
		const plan = JSON.parse(
			this.#core.complete_model_execution_plan(
				this.#encoded,
				summary.role,
				summary.tool_calls,
				summary.failed,
				summary.role === "assistant" && messageStopReason(message) === "length",
				globalSequential,
				hasSequential,
			),
		) as { state: string; tool_action: number };
		return { state: this.#accept(plan.state), toolAction: plan.tool_action };
	}

	toolsCompleted(terminate = false): MachineState {
		return this.#accept(this.#core.tools_completed(this.#encoded, terminate));
	}

	completeTools(
		messages: readonly unknown[],
		summary: { total: number; failed: number; terminate: boolean },
	): MachineState {
		const last = summarizeMessage(messages.at(-1));
		return this.#accept(
			this.#core.complete_tool_turn(
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

	completeToolOutcomes(
		messages: readonly unknown[],
		summary: { total: number; failed: number; terminating: number },
	): MachineState {
		const last = summarizeMessage(messages.at(-1));
		return this.#accept(
			this.#core.complete_tool_outcomes(
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

	queuesCompleted(options: {
		steeringMessages?: number;
		followUpMessages?: number;
		forceContinue?: boolean;
		forceEnd?: boolean;
	} = {}): MachineState {
		return this.#accept(
			this.#core.queues_completed(
				this.#encoded,
				options.steeringMessages ?? 0,
				options.followUpMessages ?? 0,
				options.forceContinue ?? false,
				options.forceEnd ?? false,
			),
		);
	}

	#accept(encoded: string): MachineState {
		const state = JSON.parse(encoded) as MachineState;
		if (state.effect === Effect.Invalid) throw new Error(`Invalid Oche machine state: ${state.error}`);
		this.#encoded = encoded;
		this.#state = state;
		return state;
	}
}

function migrateCheckpointV1(encoded: string): string {
	const decoded = JSON.parse(encoded) as unknown;
	if (typeof decoded !== "object" || decoded === null || Array.isArray(decoded)) {
		throw new Error("Invalid Oche agent checkpoint format 1 state");
	}
	const legacy = decoded as Record<string, unknown>;
	const history = Array.isArray(legacy.history) ? legacy.history : [];
	const messages = Array.isArray(legacy.messages) ? legacy.messages : [];
	return JSON.stringify({
		schema: 2,
		phase: legacy.phase,
		turn: legacy.turn,
		must_continue: legacy.must_continue,
		effect: legacy.effect,
		error: legacy.error,
		transition_count: history.length,
		message_count: messages.length,
		history: history.slice(-1),
		messages: messages.slice(-1),
		tool_batch: legacy.tool_batch,
		queues: legacy.queues,
	});
}

function summarizeMessage(message: unknown): MessageSummary {
	if (typeof message !== "object" || message === null) {
		return { role: "custom", tool_calls: 0, failed: false };
	}
	const role = "role" in message && typeof message.role === "string" ? message.role : "custom";
	const content = "content" in message && Array.isArray(message.content) ? message.content : [];
	const toolCalls = content.filter(
		(item) => typeof item === "object" && item !== null && "type" in item && item.type === "toolCall",
	).length;
	const failed =
		("stopReason" in message && (message.stopReason === "error" || message.stopReason === "aborted")) ||
		("isError" in message && message.isError === true);
	return { role, tool_calls: toolCalls, failed };
}

function summarizeMessages(messages: readonly unknown[]): { count: number; last: MessageSummary } {
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
		last_tool_calls: summary.last.tool_calls,
		last_failed: summary.last.failed,
	};
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

export async function loadOcheAgentMachine(
	moduleUrl: string,
	checkpoint?: OcheAgentCheckpoint,
): Promise<OcheAgentMachine> {
	const imported = (await import(moduleUrl)) as { default: OchePiCore };
	const machine = new OcheAgentMachine(imported.default);
	if (checkpoint) machine.restore(checkpoint);
	return machine;
}

/** Drive Oche's scheduling policy while the TypeScript host performs I/O. */
export async function runOcheAgent<TModelTurn>(
	machine: OcheAgentMachine,
	host: OcheAgentHost<TModelTurn>,
	signal?: AbortSignal,
): Promise<OcheAgentRun> {
	let state = machine.started ? machine.state : machine.start();
	let currentTurn: TModelTurn | undefined;

	while (state.effect !== Effect.Done) {
		if (signal?.aborted) return { reason: "aborted", state };

		switch (state.effect) {
			case Effect.RequestModel: {
				const response = await host.requestModel(state, signal);
				currentTurn = response.value;
				machine.observeModel(response);
				state = machine.advanceModel();
				break;
			}
			case Effect.ExecuteTools: {
				if (currentTurn === undefined) throw new Error("Oche requested tools before a model turn");
				const result = await host.executeTools(currentTurn, state, signal);
				machine.observeToolBatch({
					total: result.total ?? 0,
					failed: result.failed ?? 0,
					terminate: result.terminate ?? false,
				});
				state = machine.advanceTools();
				break;
			}
			case Effect.PollQueues: {
				const queues = await host.pollQueues(state, signal);
				machine.observeQueues(queues);
				state = machine.advanceQueues();
				break;
			}
			default:
				throw new Error(`Unknown Oche effect: ${state.effect}`);
		}
	}

	return { reason: "done", state };
}
