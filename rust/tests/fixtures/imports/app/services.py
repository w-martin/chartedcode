from __future__ import annotations

from typing import TYPE_CHECKING, Protocol

import app.base as b
from app import Engine
from app import other
from app.models import User

if TYPE_CHECKING:
    from app.core import Engine as E


class Svc(b.Base):
    users: list[User]
    engine: "Engine"
    twin: other.User
    typed: E


class Reader(Protocol):
    def read(self): ...


class FileReader:
    def read(self):
        return 1


class Child(User):
    pass


class Local(Admin):
    pass
