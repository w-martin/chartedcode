from pkg._internal import internal_fn
from pkg.util import helper


def _private_fn():
    helper(1)


def public_fn():
    _private_fn()
    internal_fn()
    helper(2)


class _PrivateClass:
    def run(self):
        helper(3)


class Public:
    def _hidden_method(self):
        helper(4)

    def visible(self):
        self._hidden_method()
        _PrivateClass().run()
        helper(5)
