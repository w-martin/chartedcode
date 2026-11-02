from uniq_pkg.ports import Port
from app.services import Reader


class FakePort:
    def go(self):
        return 1


class FakeReader:
    def read(self):
        return 2


class Unrelated:
    def go(self):
        return 3
