def a(): ...


def b(): ...


def c(): ...


def d(): ...


def cond(): ...


def open_x(): ...


def straight():
    a()
    b()


def branch(x):
    if x:
        a()
    else:
        b()
    c()


def elif_chain(x):
    if x == 1:
        a()
    elif x == 2:
        b()
    c()


def loop_for(items):
    for i in items:
        a()
    b()


def loop_while_break(n):
    while cond():
        if n:
            break
        a()
    b()


def infinite_loop():
    while True:
        a()
    b()


def try_except():
    try:
        a()
    except ValueError:
        b()
    except KeyError:
        c()
    d()


def try_finally_return():
    try:
        a()
        return 1
    finally:
        b()
    c()


def with_stmt():
    with open_x() as f:
        a()


def match_stmt(x):
    match x:
        case 1:
            a()
        case 2:
            b()
        case _:
            c()
    d()


def assert_stmt(x):
    assert x, "message"
    a()


def raise_in_try():
    try:
        raise ValueError()
    except ValueError:
        a()


def nested_def_calls():
    def inner():
        a()

    inner()


def continue_in_for(items):
    for i in items:
        if i:
            continue
        a()


def after_return():
    return a()
    b()


def for_else(items):
    for i in items:
        a()
    else:
        b()
    c()
