"""Test fixture for function extraction."""

def simple_function():
    pass

def function_with_args(a, b, c):
    pass

def function_with_types(x: int, y: str) -> bool:
    return True

def function_with_defaults(a: int = 10, b: str = "hello") -> None:
    pass

async def async_function() -> dict:
    return {}

async def async_with_args(url: str, timeout: int) -> bytes:
    return b""

def function_with_varargs(*args, **kwargs):
    pass

def function_with_typed_varargs(*args: int, **kwargs: str) -> list:
    return []

def function_with_kwonly(*, name: str, value: int) -> None:
    pass

def _private_function() -> None:
    """This is a private function."""
    pass

def __dunder_function__() -> None:
    """This is a dunder function."""
    pass

