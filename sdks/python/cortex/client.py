"""HTTP client for the Cortex API (`cortex serve`)."""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Dict, List, Mapping, Optional, Union

import httpx

DEFAULT_BASE_URL = "http://127.0.0.1:8080"


@dataclass
class ToolResult:
    """One tool invocation from a run."""

    name: str
    is_error: bool
    output: str


@dataclass
class RunResult:
    """Result of POST /v1/runs."""

    session_id: str
    run_id: str
    status: str
    turns: int
    final_message: Optional[str]
    duration_ms: int
    error: Optional[str] = None
    tool_results: List[ToolResult] = field(default_factory=list)

    @property
    def ok(self) -> bool:
        return self.status == "succeeded" and not self.error

    @classmethod
    def from_dict(cls, data: Mapping[str, Any]) -> "RunResult":
        tools = [
            ToolResult(
                name=t.get("name", ""),
                is_error=bool(t.get("is_error")),
                output=t.get("output", ""),
            )
            for t in data.get("tool_results") or []
        ]
        return cls(
            session_id=str(data["session_id"]),
            run_id=str(data["run_id"]),
            status=str(data.get("status", "")),
            turns=int(data.get("turns") or 0),
            final_message=data.get("final_message"),
            duration_ms=int(data.get("duration_ms") or 0),
            error=data.get("error"),
            tool_results=tools,
        )


class CortexError(Exception):
    """API or client error."""

    def __init__(self, message: str, *, status_code: Optional[int] = None, body: Any = None):
        super().__init__(message)
        self.status_code = status_code
        self.body = body


class CortexClient:
    """Synchronous client for a Cortex HTTP API instance.

    Example::

        with CortexClient() as client:
            print(client.health())
            result = client.run("Summarize the README", yolo=True)
            print(result.final_message)
    """

    def __init__(
        self,
        base_url: str = DEFAULT_BASE_URL,
        api_key: Optional[str] = None,
        *,
        timeout: float = 600.0,
        transport: Optional[httpx.BaseTransport] = None,
    ) -> None:
        self.base_url = base_url.rstrip("/")
        self.api_key = api_key
        headers: Dict[str, str] = {"Accept": "application/json"}
        if api_key:
            headers["Authorization"] = f"Bearer {api_key}"
        self._client = httpx.Client(
            base_url=self.base_url,
            headers=headers,
            timeout=timeout,
            transport=transport,
        )

    def close(self) -> None:
        self._client.close()

    def __enter__(self) -> "CortexClient":
        return self

    def __exit__(self, *args: Any) -> None:
        self.close()

    def _request(
        self,
        method: str,
        path: str,
        *,
        json: Any = None,
        params: Optional[Mapping[str, Any]] = None,
    ) -> Any:
        try:
            resp = self._client.request(method, path, json=json, params=params)
        except httpx.HTTPError as e:
            raise CortexError(f"HTTP request failed: {e}") from e
        if resp.status_code >= 400:
            try:
                body = resp.json()
                msg = body.get("error") or resp.text
            except Exception:
                body = resp.text
                msg = resp.text
            raise CortexError(msg, status_code=resp.status_code, body=body)
        if resp.status_code == 204 or not resp.content:
            return None
        return resp.json()

    def health(self) -> Dict[str, Any]:
        """GET /health (no auth)."""
        return self._request("GET", "/health")

    def info(self) -> Dict[str, Any]:
        """GET /v1/info."""
        return self._request("GET", "/v1/info")

    def models(self) -> List[Dict[str, Any]]:
        """GET /v1/models."""
        return self._request("GET", "/v1/models")

    def tools(self) -> List[Dict[str, Any]]:
        """GET /v1/tools."""
        return self._request("GET", "/v1/tools")

    def sessions(self, limit: int = 20) -> List[Dict[str, Any]]:
        """GET /v1/sessions."""
        return self._request("GET", "/v1/sessions", params={"limit": limit})

    def get_session(self, session_id: str) -> Dict[str, Any]:
        """GET /v1/sessions/:id."""
        return self._request("GET", f"/v1/sessions/{session_id}")

    def run(
        self,
        prompt: str,
        *,
        model: Optional[str] = None,
        session_id: Optional[str] = None,
        yolo: Optional[bool] = None,
        max_turns: Optional[int] = None,
        skills: Optional[List[str]] = None,
    ) -> RunResult:
        """POST /v1/runs — execute one agent turn/task."""
        payload: Dict[str, Any] = {"prompt": prompt}
        if model is not None:
            payload["model"] = model
        if session_id is not None:
            payload["session_id"] = session_id
        if yolo is not None:
            payload["yolo"] = yolo
        if max_turns is not None:
            payload["max_turns"] = max_turns
        if skills:
            payload["skills"] = skills
        data = self._request("POST", "/v1/runs", json=payload)
        return RunResult.from_dict(data)


class AsyncCortexClient:
    """Async variant of :class:`CortexClient`."""

    def __init__(
        self,
        base_url: str = DEFAULT_BASE_URL,
        api_key: Optional[str] = None,
        *,
        timeout: float = 600.0,
        transport: Optional[httpx.AsyncBaseTransport] = None,
    ) -> None:
        self.base_url = base_url.rstrip("/")
        self.api_key = api_key
        headers: Dict[str, str] = {"Accept": "application/json"}
        if api_key:
            headers["Authorization"] = f"Bearer {api_key}"
        self._client = httpx.AsyncClient(
            base_url=self.base_url,
            headers=headers,
            timeout=timeout,
            transport=transport,
        )

    async def close(self) -> None:
        await self._client.aclose()

    async def __aenter__(self) -> "AsyncCortexClient":
        return self

    async def __aexit__(self, *args: Any) -> None:
        await self.close()

    async def _request(
        self,
        method: str,
        path: str,
        *,
        json: Any = None,
        params: Optional[Mapping[str, Any]] = None,
    ) -> Any:
        try:
            resp = await self._client.request(method, path, json=json, params=params)
        except httpx.HTTPError as e:
            raise CortexError(f"HTTP request failed: {e}") from e
        if resp.status_code >= 400:
            try:
                body = resp.json()
                msg = body.get("error") or resp.text
            except Exception:
                body = resp.text
                msg = resp.text
            raise CortexError(msg, status_code=resp.status_code, body=body)
        if resp.status_code == 204 or not resp.content:
            return None
        return resp.json()

    async def health(self) -> Dict[str, Any]:
        return await self._request("GET", "/health")

    async def info(self) -> Dict[str, Any]:
        return await self._request("GET", "/v1/info")

    async def models(self) -> List[Dict[str, Any]]:
        return await self._request("GET", "/v1/models")

    async def tools(self) -> List[Dict[str, Any]]:
        return await self._request("GET", "/v1/tools")

    async def sessions(self, limit: int = 20) -> List[Dict[str, Any]]:
        return await self._request("GET", "/v1/sessions", params={"limit": limit})

    async def get_session(self, session_id: str) -> Dict[str, Any]:
        return await self._request("GET", f"/v1/sessions/{session_id}")

    async def run(
        self,
        prompt: str,
        *,
        model: Optional[str] = None,
        session_id: Optional[str] = None,
        yolo: Optional[bool] = None,
        max_turns: Optional[int] = None,
        skills: Optional[List[str]] = None,
    ) -> RunResult:
        payload: Dict[str, Any] = {"prompt": prompt}
        if model is not None:
            payload["model"] = model
        if session_id is not None:
            payload["session_id"] = session_id
        if yolo is not None:
            payload["yolo"] = yolo
        if max_turns is not None:
            payload["max_turns"] = max_turns
        if skills:
            payload["skills"] = skills
        data = await self._request("POST", "/v1/runs", json=payload)
        return RunResult.from_dict(data)


# Back-compat alias used by older stubs
Cortex = CortexClient
