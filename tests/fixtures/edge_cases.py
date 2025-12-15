"""Test fixture for edge case parameter types."""

# Function with typed *args (line 48 in parser.rs)
def func_with_typed_star_args(*args: tuple) -> None:
    pass

# Function with untyped keyword-only args (line 67 in parser.rs)
def func_with_untyped_kwonly(*, name, value) -> None:
    pass

# Function with typed **kwargs
def func_with_typed_kwargs(**kwargs: dict) -> None:
    pass

# Function with complex mix
def complex_func(a: int, b, *args, name: str, value, **kwargs: dict) -> bool:
    return True

