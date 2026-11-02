from __future__ import annotations

import json
import pkg.util as u
from pkg import helper
from pkg.util import Sub as S
from pkg.util import Tool
from typing import Protocol


class Port(Protocol):
    def send(self, x: int) -> None: ...


class Impl:
    def send(self, x: int) -> None:
        return None


class Other:
    def send(self, x: int) -> None:
        return None


class NoSend:
    def x(self): ...


class Service:
    _port: Port

    def __init__(self, port: Port, tool: Tool) -> None:
        self._port = port
        self._tool = Tool()
        self.t2 = tool

    def run(self, tool: Tool, items: list[Tool]):
        helper(1)
        u.other(2)
        self._port.send(3)
        self._tool.work()
        tool.work()
        items.append(tool)
        t = S()
        t.go()
        Tool.make()
        print("x")
        json.dumps(1)
        undefined_var.foo()
        self.callback()
        sorted(items, key=helper)
        isinstance(tool, Tool)
        list(map(self._port.send, [1]))


def main():
    Service(Impl(), Tool()).run(Tool(), [])


if __name__ == "__main__":
    main()
