def is_even(n):
    return n == 0 or is_odd(n - 1)


def is_odd(n):
    return n != 0 and is_even(n - 1)


def fact(n):
    return n * fact(n - 1)


def top():
    is_even(4)
    fact(3)
    is_odd(2)
