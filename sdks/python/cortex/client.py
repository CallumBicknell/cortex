# Copyright (c) 2024 Cortex Developers
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.

"""Cortex client for interacting with the Cortex runtime."""

from __future__ import annotations

from typing import Optional

import httpx

from .agent import Agent


class CortexClient:
    """A client for interacting with a Cortex runtime instance."""

    def __init__(self, base_url: str = "http://localhost:8000", api_key: Optional[str] = None):
        """Initialize the client.

        Args:
            base_url: The base URL of the Cortex REST API.
            api_key: Optional API key for authentication.
        """
        self.base_url = base_url.rstrip("/")
        self.api_key = api_key
        self._client = httpx.AsyncClient(
            base_url=self.base_url,
            headers={"Authorization": f"Bearer {api_key}"} if api_key else {},
        )

    async def run_agent(self, agent: Agent) -> None:
        """Run an agent on the Cortex runtime.

        Args:
            agent: The agent to run.
        """
        # Placeholder for actual implementation
        await agent.run()

    async def close(self) -> None:
        """Close the underlying HTTP client."""
        await self._client.aclose()

    async def __aenter__(self) -> "CortexClient":
        return self

    async def __aexit__(self, exc_type, exc_val, exc_tb) -> None:
        await self.close()