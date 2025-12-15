"""Test fixture with mixed content: functions, classes, and enums."""
from enum import Enum
from typing import Optional, List, Dict, Any

# Top-level function
def helper_function(x: int) -> int:
    return x * 2

# Async top-level function
async def fetch_data(url: str) -> Dict[str, Any]:
    return {}

# Regular class
class DataProcessor:
    data: List[int]
    _cache: Dict[str, Any]

    def __init__(self, data: List[int]):
        self.data = data
        self._cache = {}

    def process(self) -> List[int]:
        return [x * 2 for x in self.data]

    def _internal_helper(self) -> None:
        pass

    async def async_process(self) -> List[int]:
        return self.process()

# Enum
class Priority(Enum):
    LOW = 1
    MEDIUM = 2
    HIGH = 3

# Another function
def compute_result(a: int, b: int, *, operation: str = "add") -> int:
    if operation == "add":
        return a + b
    return a - b

# Private function
def _internal_compute(x: int) -> int:
    return x

# Class with complex types
class ComplexTypes:
    mapping: Dict[str, List[int]]
    optional_value: Optional[str]
    union_type: int | str

    def get_mapping(self) -> Dict[str, List[int]]:
        return self.mapping

