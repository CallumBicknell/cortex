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

"""Tool abstraction and decorator for the Cortex Python SDK."""

from __future__ import annotations

import asyncio
import functools
import inspect
from typing import Any, Callable, Dict, List, Optional


class Tool:
    """A tool that an agent can use."""

    def __init__(
        self,
        name: str,
        description: str,
        func: Callable[..., Any],
        parameters: Optional[Dict[str, Any]] = None,
    ):
        """Initialize the tool.

        Args:
            name: The name of the tool.
            description: A description of what the tool does.
            func: The underlying function that implements the tool.
            parameters: Optional JSON schema for the tool's parameters.
        """
        self.name = name
        self.description = description
        self.func = func
        self.parameters = parameters or {"type": "object", "properties": {}}

    async def execute(self, **kwargs: Any) -> Any:
        """Execute the tool with the given keyword arguments.

        Args:
            **kwargs: Arguments to pass to the tool function.

        Returns:
            The result of the tool execution.
        """
        if inspect.iscoroutinefunction(self.func):
            return await self.func(**kwargs)
        else:
            # Run synchronous function in a thread pool to avoid blocking
            loop = asyncio.get_event_loop()
            return await loop.run_in_executor(None, lambda: self.func(**kwargs))

    def __repr__(self) -> str:
        return f"Tool(name={self.name!r})"


def tool(
    *,
    name: Optional[str] = None,
    description: Optional[str] = None,
) -> Callable[[Callable[..., Any]], Tool]:
    """Decorator to convert a function into a Tool.

    Args:
        name: Optional name for the tool. Defaults to the function name.
        description: Optional description for the tool. Defaults to the function's docstring.

    Returns:
        A decorator that converts a function into a Tool.
    """
    def decorator(func: Callable[..., Any]) -> Tool:
        nonlocal name, description
        tool_name = name or func.__name__
        tool_description = description or (func.__doc__ or "").strip()
        return Tool(
            name=tool_name,
            description=tool_description,
            func=func,
        )
    return decorator