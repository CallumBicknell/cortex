# Constraints

The following are project constraints and are not optional.

- The runtime must be provider agnostic.
- No component may depend directly on a specific LLM.
- All runtime state must be serializable.
- Every action must emit an event.
- All long-running operations must support cancellation.
- SQLite must be supported.
- PostgreSQL should be supported.
- Plugins must not require recompiling the runtime.
- APIs should be versioned.
- Avoid breaking changes where practical.
- The runtime must be usable headlessly.
- The dashboard is optional and must never be required.
