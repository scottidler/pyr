"""Test fixture with non-BitOr binary operations in type annotations."""

# Using Add operator (not valid Python, but exercises the parser)
def func_with_weird_annotation(x: int) -> None:
    pass

# Complex type that triggers fallback expression handling
class WeirdClass:
    # Lambda is an unsupported expression type
    weird_field = lambda x: x

