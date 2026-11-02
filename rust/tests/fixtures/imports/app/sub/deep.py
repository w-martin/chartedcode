from .. import models
from ..base import Base as RootBase
from . import helper


class Deep(models.Admin):
    pass


class Deeper(RootBase):
    pass


class H(helper.Mixin):
    pass
