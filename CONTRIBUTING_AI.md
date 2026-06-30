# Contributing Guide for AI Agents

This document provides guidance for AI assistants (like GitHub Copilot, Codex, etc.) that are contributing to the Cortex project.

## Principles

1. **Follow the existing CONTRIBUTING.md** - All guidelines in the main contributing guide apply to AI agents as well.
2. **Maintain code quality** - Ensure any code generated adheres to the project's formatting (rustfmt), linting (clippy), and testing standards.
3. **Make small, focused changes** - Keep changes minimal and focused on a single goal to facilitate review.
4. **Write clear commit messages** - Use conventional commits format (e.g., `feat(component): description`).
5. **Update documentation** - If your changes affect documentation, update the relevant files.
6. **Run tests** - Ensure all tests pass before submitting changes.
7. **Respect the project's architecture** - Follow the established patterns and avoid introducing unnecessary complexity.

## Specific Considerations for AI Agents

- When generating Rust code, prefer explicit types and avoid overuse of type inference where it might reduce clarity.
- Ensure that any generated code handles error cases appropriately.
- Avoid generating code that introduces blocking calls in asynchronous contexts unless necessary.
- When modifying the Python SDK, follow PEP 8 and use type hints.

## Process

1. Fork the repository.
2. Create a feature branch from `main`.
3. Make your changes.
4. Ensure cargo fmt, clippy, and tests pass.
5. Commit with a descriptive conventional commit message.
6. Push to your fork and open a pull request.

Thank you for contributing to Cortex!
