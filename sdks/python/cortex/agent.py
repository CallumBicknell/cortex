"""Lightweight agent helper that runs prompts via the Cortex HTTP API."""

from __future__ import annotations

from typing import Any, List, Optional, Sequence

from .client import AsyncCortexClient, CortexClient, RunResult
from .tool import Tool


class Agent:
    """Convenience wrapper: holds metadata and runs prompts through a client.

    Remote tool execution uses the server's builtin tools (not local Python
    callables). Local :class:`Tool` instances are kept for documentation /
    future remote registration.
    """

    def __init__(
        self,
        name: str = "agent",
        description: Optional[str] = None,
        tools: Optional[List[Tool]] = None,
        *,
        client: Optional[CortexClient] = None,
        model: Optional[str] = None,
        skills: Optional[Sequence[str]] = None,
        yolo: bool = True,
        max_turns: int = 32,
    ) -> None:
        self.name = name
        self.description = description
        self.tools = list(tools or [])
        self.client = client
        self.model = model
        self.skills = list(skills or [])
        self.yolo = yolo
        self.max_turns = max_turns
        self.session_id: Optional[str] = None

    def add_tool(self, tool: Tool) -> None:
        self.tools.append(tool)

    def run(self, prompt: str, **kwargs: Any) -> RunResult:
        """Run a prompt (sync). Requires ``client`` to be set."""
        if self.client is None:
            raise RuntimeError("Agent.client is not set; pass client=CortexClient(...)")
        result = self.client.run(
            prompt,
            model=kwargs.get("model", self.model),
            session_id=kwargs.get("session_id", self.session_id),
            yolo=kwargs.get("yolo", self.yolo),
            max_turns=kwargs.get("max_turns", self.max_turns),
            skills=list(kwargs.get("skills", self.skills)) or None,
        )
        self.session_id = result.session_id
        return result

    async def arun(self, prompt: str, client: Optional[AsyncCortexClient] = None, **kwargs: Any) -> RunResult:
        """Run a prompt (async)."""
        c = client
        if c is None:
            raise RuntimeError("pass an AsyncCortexClient to arun()")
        result = await c.run(
            prompt,
            model=kwargs.get("model", self.model),
            session_id=kwargs.get("session_id", self.session_id),
            yolo=kwargs.get("yolo", self.yolo),
            max_turns=kwargs.get("max_turns", self.max_turns),
            skills=list(kwargs.get("skills", self.skills)) or None,
        )
        self.session_id = result.session_id
        return result

    def __repr__(self) -> str:
        return f"Agent(name={self.name!r}, tools={len(self.tools)}, session={self.session_id!r})"
