from typing import Protocol


class Port(Protocol):
    def go(self): ...
