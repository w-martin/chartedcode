def helper(x):
    return x


def other(x):
    return x


class Tool:
    def __init__(self):
        pass

    def work(self):
        return helper(1)

    def run(self):
        self.work()

    @staticmethod
    def make():
        return Tool()


class Sub(Tool):
    def work(self):
        super().work()

    def go(self):
        self.run()
