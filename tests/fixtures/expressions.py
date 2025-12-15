"""Test fixture for expression parsing."""
from typing import Optional, List, Dict, Tuple

# Constants with various types
NONE_CONST = None
TRUE_CONST = True
FALSE_CONST = False
STRING_CONST = "hello"
INT_CONST = 42
FLOAT_CONST = 3.14
ELLIPSIS_CONST = ...

# List type annotations
def func_with_list() -> [int, str]:
    pass

# Tuple annotations
def func_with_tuple(args: (int, str, bool)) -> None:
    pass

# Call expressions in annotations
def func_with_callable(callback: Callable[[int], str]) -> None:
    pass

# Binary operations (non-BitOr)
def func_with_add_type(x: int + str) -> None:
    pass

# Complex nested types
class ComplexAnnotations:
    nested: Dict[str, List[Tuple[int, Optional[str]]]]

    def method_with_complex_return(self) -> Dict[str, List[int]]:
        return {}

