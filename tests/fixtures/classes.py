"""Test fixture for class extraction."""
from typing import Optional, List

class SimpleClass:
    pass

class ClassWithBase(object):
    pass

class ClassWithMultipleBases(dict, list):
    pass

class ClassWithFields:
    name: str
    value: int
    items: List[str]
    _private_field: int

    def __init__(self, name: str, value: int):
        self.name = name
        self.value = value

class ClassWithMethods:
    def public_method(self) -> None:
        pass

    def _private_method(self) -> None:
        pass

    async def async_method(self, data: bytes) -> dict:
        return {}

    @staticmethod
    def static_method(x: int) -> int:
        return x * 2

    @classmethod
    def class_method(cls, name: str) -> "ClassWithMethods":
        return cls()

class ClassWithFieldsAndMethods:
    count: int = 0
    name: str

    def get_count(self) -> int:
        return self.count

    def set_name(self, name: str) -> None:
        self.name = name

class _PrivateClass:
    """A private class."""
    value: int

    def get_value(self) -> int:
        return self.value

