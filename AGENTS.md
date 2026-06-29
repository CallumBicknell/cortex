# AGENTS.md

> Instructions for any AI coding agent (Claude Code, Codex CLI, Cursor, Aider, OpenCode, Gemini CLI, Copilot Workspace, etc.) working in this repository.
>
> These instructions define the engineering standards for this project and must be followed unless the user explicitly overrides them.

---

# Core Principles

The primary objective is **correctness, maintainability, and long-term quality**.

Optimize for:

* Correctness
* Readability
* Maintainability
* Reliability
* Testability
* Determinism
* Simplicity

Do **not** optimize for:

* Clever code
* Short code
* Premature optimization
* Unnecessary abstractions

Always prefer code that another engineer can understand in six months.

---

# Engineering Philosophy

Think before coding.

Design before implementing.

Implement before optimizing.

Measure before optimizing further.

Every change should make the repository better.

---

# Planning

Before making any changes:

1. Understand the request completely.
2. Read all relevant code.
3. Understand existing architecture.
4. Identify affected modules.
5. Consider alternative implementations.
6. Choose the simplest maintainable solution.

If requirements are ambiguous:

Stop.

Ask.

Do not guess.

---

# Large Features

If work involves:

* multiple subsystems
* architectural changes
* new dependencies
* public API changes
* > 500 LOC
* significant refactoring

Do NOT immediately start coding.

Instead:

1. Produce an architecture proposal.
2. Explain tradeoffs.
3. Break into milestones.
4. Implement one milestone at a time.

---

# Scope Control

Only modify code required for the requested task.

Do not:

* refactor unrelated modules
* rename files unnecessarily
* update unrelated dependencies
* perform "drive-by" cleanup

Mention unrelated improvements as suggestions instead.

---

# Architecture Principles

Prefer:

* composition over inheritance
* dependency injection
* explicit interfaces
* modular design
* immutable data where practical
* pure functions where possible
* small focused modules
* deterministic behaviour

Avoid:

* singleton abuse
* hidden globals
* circular dependencies
* giant classes
* giant functions
* tightly coupled modules
* magic values

---

# Code Quality

Functions should generally perform one responsibility.

Prefer:

* descriptive names
* early returns
* explicit logic

Avoid:

* deep nesting
* excessive comments explaining bad code
* duplicated logic

Code should explain itself.

---

# Dependencies

Before adding a dependency ask:

Can this be solved using the standard library?

If adding a dependency:

* explain why
* choose mature libraries
* avoid abandoned packages
* avoid duplicate functionality
* avoid unnecessary transitive dependencies

Never replace an existing dependency without justification.

---

# Configuration

Never hardcode:

* API keys
* credentials
* ports
* URLs
* filesystem paths
* secrets

Everything configurable should be configurable.

Provide sensible defaults.

---

# Error Handling

Never ignore errors.

Never swallow exceptions.

Never silently continue.

Errors should:

* contain useful context
* be actionable
* propagate correctly
* be logged once

Do not panic/unwrap unless impossible to recover.

---

# Logging

Logs should be useful.

Log:

* startup
* shutdown
* warnings
* failures
* retries
* important lifecycle events

Never log:

* passwords
* API keys
* tokens
* secrets
* personal data

Avoid excessive logging.

---

# Performance

Don't optimize blindly.

Measure first.

Avoid:

* unnecessary allocations
* repeated filesystem scans
* repeated network calls
* repeated database queries
* repeated LLM calls
* unnecessary cloning

Prefer streaming for large data.

---

# Security

Validate:

* user input
* paths
* URLs
* filenames
* configuration

Never:

* commit secrets
* bypass authentication
* disable TLS
* disable validation

Escape shell commands.

Treat external input as untrusted.

---

# Documentation

Update documentation whenever changing:

* setup
* configuration
* CLI
* APIs
* architecture
* workflows

Public APIs require documentation.

---

# Comments

Comment:

* why

Not:

* what

Avoid obvious comments.

Delete stale comments.

---

# Formatting

Always use project-configured formatting.

Detect tools from the repository.

Examples:

Rust

* cargo fmt

Python

* ruff format
* black

TypeScript

* prettier

Go

* gofmt

Never invent formatting rules.

---

# Linting

Run configured linters.

Auto-fix first.

Resolve remaining issues manually.

A task is not complete while new lint errors exist.

---

# Testing

Every change should include appropriate tests.

Minimum:

* happy path
* edge cases
* regressions

Run the full test suite before committing.

Never commit failing tests.

If failures already existed:

Document them.

Do not silently fix unrelated tests.

---

# Git Workflow

Never work directly on main unless instructed.

Create feature branches.

Naming:

feat/...

fix/...

refactor/...

docs/...

test/...

perf/...

chore/...

---

# Commits

One logical change per commit.

Large features may use multiple logical commits.

Never mix unrelated work.

Use Conventional Commits.

Examples:

feat(runtime): add event dispatcher

fix(memory): prevent duplicate checkpoint

refactor(loop): simplify scheduler

Commit messages should explain WHY.

---

# Before Commit

Ensure:

✓ Formatting clean

✓ Lint clean

✓ Tests pass

✓ Documentation updated

✓ No debug code

✓ No TODOs

✓ No commented-out code

✓ No secrets

✓ No unnecessary files

---

# Push Policy

Automatically push only if:

* tests pass
* lint passes
* formatting passes
* feature branch
* no divergence

Never force push.

Never push directly to main.

---

# Pull Requests

If supported:

Create a PR automatically.

Include:

* summary
* motivation
* testing
* limitations

Never merge without instruction.

---

# Refactoring

Only refactor:

* directly related code
* blocking implementation

Large refactors should be separate tasks.

---

# Public APIs

Changing a public interface requires:

* documentation
* migration notes
* tests
* justification

Prefer backwards compatibility.

---

# Repository Hygiene

Never leave:

* debug prints
* temporary files
* commented-out code
* unused imports
* dead code

---

# AI-Specific Rules

Design for:

* provider independence
* deterministic execution
* replayability
* observability
* structured events
* explicit state
* durable execution

Avoid tight coupling to any model vendor.

---

# Runtime Design

Prefer:

event-driven systems

state machines

explicit transitions

typed events

typed state

modular plugins

Avoid:

hidden state

implicit behaviour

global mutable state

---

# Tooling

Detect tooling automatically.

Prefer project scripts.

Order:

Makefile

justfile

package.json

cargo

pyproject

language defaults

---

# Decision Making

If multiple implementations exist:

Choose the one with:

1. highest maintainability

2. least complexity

3. easiest testing

4. lowest long-term cost

Not necessarily the shortest.

---

# When Unsure

Stop.

Explain uncertainty.

Ask.

Never invent behaviour.

---

# Definition of Done

A task is complete only when:

* requirements implemented
* tests pass
* formatting passes
* lint passes
* documentation updated
* architecture respected
* no regressions introduced
* commit created

---

# Review Checklist

Before finishing verify:

* Correctness
* Readability
* Simplicity
* Maintainability
* Tests
* Documentation
* Security
* Performance
* Error handling
* Logging
* Formatting
* Lint
* Public APIs
* Dependencies

---

# Final Report

Always finish with:

## Summary

What changed.

## Files

Files modified.

## Tests

Commands run.

Results.

## Formatting

Status.

## Lint

Status.

## Documentation

Updated or not.

## Commit

Commit hash.

Commit message.

## Push

Whether pushed.

PR created if applicable.

## Notes

Assumptions.

Known limitations.

Follow-up suggestions.

---

# Things Never To Do

Never:

* guess requirements
* fabricate APIs
* fabricate test results
* fabricate benchmark results
* claim tests passed when not run
* claim lint passed when not run
* hide failures
* ignore warnings
* commit broken code
* disable tests to make them pass
* add dependencies without reason
* rewrite unrelated code
* remove functionality without instruction

Honesty is mandatory.

Always accurately report what was done and what could not be completed.

---

# Goal

Leave the repository in a better state than it was found while respecting the requested scope.

Every change should be something a senior engineer would confidently approve during code review.

