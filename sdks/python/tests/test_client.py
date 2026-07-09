"""Unit tests for the Cortex Python client (httpx mock transport)."""

from __future__ import annotations

import json

import httpx
import pytest

from cortex import CortexClient, CortexError, RunResult, tool


def _handler(request: httpx.Request) -> httpx.Response:
    if request.url.path == "/health":
        return httpx.Response(200, json={"status": "ok", "version": "0.1.0"})
    if request.url.path == "/v1/info":
        auth = request.headers.get("Authorization")
        if auth != "Bearer secret":
            return httpx.Response(401, json={"error": "unauthorized", "code": "unauthorized"})
        return httpx.Response(
            200,
            json={
                "version": "0.1.0",
                "workspace": "/tmp",
                "database": "/tmp/db",
                "models_config": "/tmp/m.toml",
                "auth_required": True,
                "default_yolo": True,
                "default_max_turns": 32,
            },
        )
    if request.url.path == "/v1/models":
        return httpx.Response(
            200,
            json=[{"alias": "default", "provider_id": "mock", "model": "mock-model"}],
        )
    if request.url.path == "/v1/tools":
        return httpx.Response(
            200,
            json=[{"name": "read_file", "description": "Read a file"}],
        )
    if request.url.path == "/v1/runs" and request.method == "POST":
        body = json.loads(request.content.decode())
        assert "prompt" in body
        return httpx.Response(
            200,
            json={
                "session_id": "11111111-1111-1111-1111-111111111111",
                "run_id": "22222222-2222-2222-2222-222222222222",
                "status": "succeeded",
                "turns": 1,
                "final_message": f"echo:{body['prompt']}",
                "duration_ms": 12,
                "error": None,
                "tool_results": [],
            },
        )
    return httpx.Response(404, json={"error": "not found"})


def test_health_and_run():
    transport = httpx.MockTransport(_handler)
    with CortexClient("http://test", transport=transport) as client:
        h = client.health()
        assert h["status"] == "ok"
        models = client.models()
        assert models[0]["alias"] == "default"
        tools = client.tools()
        assert tools[0]["name"] == "read_file"
        result = client.run("hello world", yolo=True)
        assert isinstance(result, RunResult)
        assert result.ok
        assert result.final_message == "echo:hello world"


def test_auth_error():
    transport = httpx.MockTransport(_handler)
    with CortexClient("http://test", transport=transport) as client:
        with pytest.raises(CortexError) as ei:
            client.info()
        assert ei.value.status_code == 401

    with CortexClient("http://test", api_key="secret", transport=transport) as client:
        info = client.info()
        assert info["auth_required"] is True


def test_tool_decorator():
    @tool(name="add", description="add two numbers")
    def add(a: int, b: int) -> int:
        return a + b

    assert add.name == "add"
    import asyncio

    assert asyncio.run(add.execute(a=1, b=2)) == 3
