"""Test fixture for enum extraction."""
from enum import Enum, IntEnum, StrEnum, Flag, auto

class Color(Enum):
    RED = 1
    GREEN = 2
    BLUE = 3

class Status(IntEnum):
    PENDING = 0
    RUNNING = 1
    COMPLETE = 2
    FAILED = 3

class Direction(StrEnum):
    NORTH = "north"
    SOUTH = "south"
    EAST = "east"
    WEST = "west"

class Permissions(Flag):
    READ = auto()
    WRITE = auto()
    EXECUTE = auto()

# This is NOT an enum, should not be extracted
class NotAnEnum:
    pass

class AlsoNotAnEnum(object):
    pass

