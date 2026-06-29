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

"""Agent abstraction for the Cortex Python SDK."""

from __future__ import annotations

from typing import Any, Callable, List, Optional

from .tool import Tool


class Agent:
    """An autonomous agent that runs in the Cortex runtime."""

    def __init__(
        self,
        name: str,
        description: Optional[str] = None,
        tools: Optional[List[Tool]] = None,
    ):
        """Initialize the agent.

        Args:
            name: The name of the agent.
            description: Optional description of the agent.
            tools: Optional list of tools the agent can use.
        """
        self.name = name
        self.description = description
        self.tools = tools or []

    def add_tool(self, tool: Tool) -> None:
        """Add a tool to the agent.

        Args:
            tool: The tool to add.
        """
        self.tools.append(tool)

    async def run(self) -> None:
        """Run the agent's main loop.

        This is a placeholder implementation.
        """
        # In a real implementation, this would connect to the Cortex runtime
        # and execute the agent loop.
        print(f"Agent {self.name} is running with {len(self.tools)} tools.")
        # Simulate some work
        for tool in self.tools:
            print(f"  - Tool: {tool.name}")

    def __repr__(self) -> str:
        return f"Agent(name={self.name!r}, tools={len(self.tools)})"