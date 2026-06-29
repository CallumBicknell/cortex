This is one of the biggest shifts happening in agent engineering during 2026.

A year ago everyone talked about **prompt engineering**. Then it became **context engineering**. Now the conversation has shifted towards **loop engineering** and **harness engineering**.

The common idea across nearly all of the articles is:

> **The LLM is no longer the interesting part. The runtime around it is.** ([OpenReview][1])

---

# The evolution

```
2023
┌─────────┐
│ Prompt  │
└─────────┘

↓

2024
┌────────────┐
│ Context    │
│ Engineering│
└────────────┘

↓

2025-2026
┌────────────┐
│ Harness    │
│ Engineering│
└────────────┘

↓

Next
┌────────────┐
│ Loop        │
│ Engineering │
└────────────┘
```

People discovered that changing the prompt gives maybe a few percent improvement.

Changing the **runtime** gives dramatically larger improvements.

---

# What actually is Loop Engineering?

Think of an AI agent as this:

```
User

↓

LLM

↓

Answer
```

That isn't an agent.

A modern agent is:

```
Observe

↓

Plan

↓

Reason

↓

Choose Tool

↓

Execute

↓

Observe Result

↓

Update Memory

↓

Reflect

↓

Continue?

↓

Repeat
```

That repeating cycle...

```
while !done {

    observe()

    think()

    act()

    evaluate()

}
```

...is the **agent loop**.

Loop engineering is designing this loop.

Not the prompt.

Not the model.

The loop.

---

# Harness Engineering vs Loop Engineering

They're related but different.

## Harness

Everything surrounding the model.

Think operating system.

Responsible for

* permissions
* tools
* sandbox
* memory
* filesystem
* browser
* logging
* retries
* checkpoints
* observability
* verification
* approvals
* persistence

A harness is infrastructure.

---

## Loop

The runtime algorithm.

Responsible for

```
How many times do we think?

When do we stop?

Should we retry?

Should we use another tool?

Should we ask another agent?

Should we summarize memory?

Should we rollback?

Should we branch?

Should we sleep?

Should we verify?

Should we continue?
```

Loop engineering is behaviour.

---

# Modern agent architecture

A production agent today looks more like:

```
                 User

                  │

          Task Interpreter

                  │

      ┌──────── Planner ────────┐
      │                         │
      ▼                         │
   Main Loop                    │
      │                         │
      ▼                         │
Context Builder                 │
      │                         │
      ▼                         │
LLM Invocation                  │
      │                         │
      ▼                         │
Tool Selection                  │
      │                         │
      ▼                         │
Execute Tool                    │
      │                         │
      ▼                         │
Validate Output                 │
      │                         │
      ▼                         │
Update Memory                   │
      │                         │
      ▼                         │
Reflection                      │
      │                         │
      └──── Continue? ──────────┘

```

The LLM is only one box.

---

# The "best" loops are hierarchical

Instead of

```
Think

Act

Think

Act
```

people are moving towards

```
Mission

↓

Planner

↓

Executor

↓

Verifier

↓

Critic

↓

Planner

↓

Executor

↓

Verifier

↓

Done
```

Multiple loops.

Each specialized.

---

# Loop state

The loop usually carries state.

Example

```python
state = {

    goal,

    current_step,

    completed_steps,

    pending_tasks,

    failed_tasks,

    observations,

    memory,

    context,

    budget,

    retries,

    tool_history,

    messages,

    artifacts,

    checkpoints,

}
```

Every iteration updates state.

---

# Event-driven loops

Instead of

```
while True:
```

modern agents often use events.

```
TaskStarted

↓

NeedContext

↓

NeedTool

↓

ToolFinished

↓

NeedReflection

↓

NeedApproval

↓

Continue

↓

Done
```

Everything becomes events.

Much easier to debug.

---

# Deterministic loops

One thing nearly every paper discusses is making loops deterministic.

Instead of

```
Think forever...
```

they create explicit phases.

```
PLAN

↓

EXECUTE

↓

VERIFY

↓

REFLECT

↓

END
```

Each phase has constraints.

---

# Why observability matters

Traditional AI:

```
Input

↓

LLM

↓

Output
```

Impossible to debug.

Loop engineering logs every transition.

```
Loop #4

Reasoning

↓

Selected Tool

↓

Arguments

↓

Output

↓

Reflection

↓

Memory Update

↓

Next Decision
```

Now you can replay an agent like replaying a server request. Recent work emphasizes observability, traceability, and evidence-backed decisions as core harness responsibilities rather than optional extras. ([arXiv][2])

---

# Verification loops

One huge trend:

Never trust the first answer.

```
Plan

↓

Write Code

↓

Compile

↓

Run Tests

↓

Lint

↓

Evaluate

↓

Fix

↓

Run Again

↓

Done
```

The loop owns verification.

Not the model.

---

# Memory is becoming loop-native

Instead of

```
Conversation history
```

modern loops have

Short-term

```
Current task
```

↓

Working memory

```
Current findings
```

↓

Long-term

```
Knowledge graph
```

↓

Semantic search

↓

Artifact storage

↓

Execution history

Memory becomes part of the loop itself rather than just appended chat history. ([arXiv][3])

---

# The future

Many researchers now argue the next competitive advantage isn't:

* bigger models
* better prompts
* more context

It's better runtimes.

Better loops.

Better harnesses.

In other words:

```
LLM

↓

Agent Runtime

↓

Harness

↓

Loop

↓

Reliability
```

The runtime determines whether the same model succeeds or fails on long, complex tasks. Several recent papers even show significant gains by evolving or redesigning the harness while keeping the underlying model fixed. ([arXiv][2])

## If you're building your own system

Given your plan to build an agent harness from scratch rather than wrap an existing framework, I'd architect it around a small event-driven core:

* **Kernel** – owns the main event loop.
* **Scheduler** – decides what executes next (tool, sub-agent, reflection, etc.).
* **Context Manager** – assembles the prompt from memory and current state.
* **Tool Runtime** – executes tools with permissions, timeouts, and retries.
* **State Store** – maintains goals, progress, artifacts, budgets, and checkpoints.
* **Memory Manager** – handles short-term and long-term memory.
* **Verifier** – validates outputs with tests, linting, assertions, or other checks.
* **Observer** – records every state transition, tool call, and decision for replay and debugging.
* **Policy Engine** – enforces approvals, budgets, safety rules, and loop termination conditions.

That separation gives you a clean, extensible architecture where the **loop is the orchestration engine**, the **harness is the runtime substrate**, and the **LLM is simply one component within the system**.

[1]: https://openreview.net/forum?id=3hXEPbG0dh&utm_source=chatgpt.com "Agent Harness Engineering: A Survey | OpenReview"
[2]: https://arxiv.org/abs/2604.25850?utm_source=chatgpt.com "Agentic Harness Engineering: Observability-Driven Automatic Evolution of Coding-Agent Harnesses"
[3]: https://arxiv.org/abs/2605.13357?utm_source=chatgpt.com "AI Harness Engineering: A Runtime Substrate for Foundation-Model Software Agents"

---

Based on where the ecosystem is heading, I would **not** tell an agent to build "yet another LangGraph clone."

Instead, I'd build something closer to an **agent operating system**—a runtime that happens to execute LLM agents.

Recent industry consensus is converging on the idea that the runtime (loop + state + memory + durability + observability) matters more than the individual model. Modern harnesses separate the **execution runtime** from the **agent logic**, making it easier to swap models, tools, and workflows independently. ([LangChain Docs][1])

---

# Language choice

I would choose:

## Core Runtime

**Rust**

Reasons:

* extremely fast
* memory safe
* async is excellent
* great for long-running daemons
* plugin system
* sandboxing
* event bus
* durable services
* WebSocket server
* HTTP server
* task scheduler

The runtime itself shouldn't depend on Python.

---

## SDK

Python

This is where users write

```python
@tool
def search():
    ...
```

or

```python
class MyAgent(Agent):
    ...
```

Python has the best AI ecosystem.

---

## Optional SDK

TypeScript

Later.

---

# Libraries I'd use

## Rust

Tokio

* async runtime

Axum

* REST API

Tonic

* gRPC

Serde

* serialization

SQLx

* database

SQLite

* default storage

Postgres

* production

DashMap

* concurrent cache

Tracing

* logging

OpenTelemetry

* traces

UUID

Chrono

Anyhow

This gives an extremely solid foundation.

---

# Python SDK

Pydantic

For everything.

Not optional.

Every message

Every tool

Every event

Every state

Every config

Every response

should be validated.

Pydantic has become a cornerstone for typed agent development, and its ecosystem now includes a dedicated harness package for reusable agent capabilities. ([pydantic.dev][2])

---

# Agent Loop

Never

```python
while True:
```

Instead

```text
Task

↓

Planner

↓

Executor

↓

Verifier

↓

Reflection

↓

Memory

↓

Scheduler

↓

Continue?
```

Everything becomes events.

---

# Architecture

```text
               CLI

                │

        HTTP / WebSocket

                │

       Runtime Kernel

                │

        Event Dispatcher

──────────────────────────────────

Scheduler

Planner

Loop Engine

Memory Manager

Context Manager

Verifier

Tool Registry

Artifact Manager

Session Manager

Prompt Builder

Model Manager

Plugin Manager

Metrics

Tracing

Checkpoint Manager

Approval Manager

──────────────────────────────────

        Provider Layer

──────────────────────────────────

OpenAI

Anthropic

Gemini

OpenRouter

Ollama

LM Studio

vLLM

Custom
```

---

# Storage

SQLite first.

Postgres second.

Redis optional.

Never require Redis.

---

# Event Bus

Everything should emit events.

```text
TaskStarted

↓

PlanCreated

↓

ToolRequested

↓

ToolStarted

↓

ToolFinished

↓

ModelCalled

↓

MemoryUpdated

↓

ReflectionStarted

↓

ReflectionFinished

↓

CheckpointSaved

↓

TaskCompleted
```

Nothing should happen silently.

---

# Plugin System

Everything should be a plugin.

Models

Memory

Storage

Prompts

Tools

Verifiers

Context

Loops

Schedulers

Policies

Output parsers

Approvals

---

# Built-in tools

Filesystem

Browser

Shell

Python execution

Docker

Git

GitHub

Web Search

HTTP

Database

MCP

Image

PDF

Markdown

JSON

YAML

Diff

Terminal

These should all share a common interface.

---

# Multi-Agent

Don't hardcode.

Make agents themselves plugins.

```text
Planner

↓

Coder

↓

Reviewer

↓

Researcher

↓

Security

↓

Verifier
```

---

# Memory

Separate it.

```text
Working Memory

↓

Session Memory

↓

Long-term Memory

↓

Knowledge Base

↓

Artifacts

↓

Checkpoint History
```

---

# Observability

Every iteration should record:

```text
Loop #

Timestamp

Current Goal

Tokens

Cost

Duration

Prompt

Response

Tools

Memory Used

Files Changed

Reason

Reflection

State Changes
```

Then replay later.

---

# Verification

Every action can optionally pass through

```text
Verifier

↓

Approved?

↓

Continue

↓

Retry

↓

Rollback

↓

Ask User
```

---

# Durable execution

Steal ideas from Temporal.

Agent crashes?

Resume.

Machine reboots?

Resume.

Container dies?

Resume.

Power outage?

Resume.

Recent guidance consistently highlights durable execution as one of the defining capabilities of production agent runtimes. ([LangChain Docs][1])

---

# Prompt for your coding agent

```text
You are a principal systems architect and senior Rust/Python engineer.

Your task is to design and implement a production-grade open-source Agent Runtime, Harness, and Loop Engine.

This is NOT another LangGraph clone, CrewAI clone, or wrapper around an existing framework.

The objective is to build an "Agent Operating System" that executes autonomous AI agents with reliability, observability, durability, extensibility, and high performance.

========================

Core philosophy

========================

The LLM is only one component.

The runtime is the product.

Everything should revolve around:

• deterministic execution
• event-driven architecture
• explicit state transitions
• durable execution
• checkpoints
• replayability
• observability
• verification
• extensibility
• plugin architecture

The runtime should function similarly to an operating system for AI agents.

========================

Technology stack

========================

Core Runtime:
- Rust (stable)
- Tokio
- Axum
- SQLx
- SQLite by default
- PostgreSQL support
- Serde
- Tracing
- OpenTelemetry
- UUID
- Chrono

SDK:
- Python
- Pydantic
- AsyncIO

Future SDK:
- TypeScript

========================

Architecture

========================

Implement clean modules including:

Kernel
Scheduler
Loop Engine
Planner
Context Manager
Memory Manager
Tool Registry
Plugin Manager
Model Provider Manager
Session Manager
Checkpoint Manager
Artifact Manager
Verification Engine
Policy Engine
Approval Engine
Metrics
Tracing
Configuration

The runtime must communicate internally using an event bus.

========================

Loop

========================

Implement a deterministic execution loop.

Observe

↓

Plan

↓

Execute

↓

Verify

↓

Reflect

↓

Update Memory

↓

Checkpoint

↓

Continue?

Never implement a simple while(true) loop.

Use explicit state machines and event-driven transitions.

========================

Features

========================

Implement:

- model abstraction
- provider abstraction
- plugin system
- MCP integration
- filesystem tools
- shell execution
- docker execution
- browser automation
- web search
- HTTP tools
- artifact storage
- replayable sessions
- checkpoints
- long-term memory
- working memory
- session memory
- verification pipeline
- retry policies
- rollback
- human approval hooks
- tracing
- metrics
- structured logging
- WebSocket streaming
- REST API
- CLI
- configuration system

========================

Engineering standards

========================

Everything must be:

typed

async

modular

unit tested

integration tested

benchmarkable

documented

dependency injected

extensible

========================

Output

========================

Do NOT immediately write code.

Instead:

1. Produce a complete architecture document.
2. Design every subsystem.
3. Explain design decisions and trade-offs.
4. Define the project structure.
5. Design interfaces and traits.
6. Define events.
7. Define state objects.
8. Define plugin APIs.
9. Design persistence.
10. Design loop execution.
11. Design checkpoints.
12. Design observability.
13. Design verification.
14. Design configuration.
15. Produce a complete development roadmap.
16. Break implementation into small milestones.
17. Only after the architecture is finalized should implementation begin.

Prioritize maintainability, extensibility, and long-term production readiness over short-term simplicity.
```

This approach gives you something closer to an **operating system for agents** than a framework: a minimal, durable execution kernel with pluggable loops, memories, models, and tools, rather than an opinionated agent implementation.

[1]: https://docs.langchain.com/oss/python/concepts/products?utm_source=chatgpt.com "Frameworks, runtimes, and harnesses - Docs by LangChain"
[2]: https://pydantic.dev/docs/ai/harness/overview?utm_source=chatgpt.com "Overview | Pydantic Docs"

