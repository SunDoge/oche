# Agent runtime

`agent/pi_core.oche` is the first incremental pi-agent-core replacement. Oche
owns the phase, turn number, continuation rule, and next effect. The TypeScript
driver performs model, tool, queue, and cancellation I/O, then reports each
result back to Oche. State crosses the initial primitive JS ABI as checked
JSON; the acceptance test covers plain responses, tool calls, steering,
follow-ups, termination, model failure, and invalid transitions.

`tests/run_pi_mono_acceptance.py` additionally compiles that scheduler,
injects it through pi-agent-core's `AgentLoopStateMachine` option, and runs a
complete model/tool/model loop against the local pi-mono checkout. Set `PI_MONO`
when the checkout is outside the default sibling repository layout.
