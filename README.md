# pyr

**Fast Python codebase analysis for agentic LLMs**

[![CI](https://img.shields.io/badge/coverage-90%25-brightgreen)]()
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange)]()
[![License](https://img.shields.io/badge/license-MIT-blue)]()

`pyr` is a blazing-fast Rust CLI tool that provides structured queries about Python codebases. Designed for LLM agents that need to understand project structure without reading every file.

## Features

- 🚀 **Fast** — Parallel parsing with Rayon, pure Rust implementation
- 📦 **No Python Required** — Uses `rustpython-parser`, no Python runtime needed
- 🔍 **Smart Pattern Matching** — Cascading match logic (prefix → contains, case-sensitive → insensitive)
- 🔐 **Visibility Filtering** — Filter by public/private (`_` prefix convention)
- 📄 **Flexible Output** — YAML by default, JSON for pipelines
- 🎯 **Agent-Friendly** — Structured output optimized for LLM token efficiency

## Installation

```bash
# From source
cargo install --path .

# Or build directly
cargo build --release
```

## Quick Start

```bash
# Analyze current directory
pyr function                    # List all functions
pyr class                       # List all classes
pyr enum                        # List all enums
pyr module                      # Show module structure
pyr dump                        # Everything combined

# Analyze specific targets
pyr -t src/ function            # Analyze src/ directory
pyr -t app.py function          # Analyze single file
pyr -t src/ -t tests/ function  # Multiple targets
```

## Subcommands

### `function` — List Functions

Extract all top-level function definitions with signatures and line numbers.

```bash
pyr function [PATTERN...] [--public | --private]
```

**Example:**
```bash
$ pyr -t myapp/ function
```
```yaml
files:
  myapp/utils.py:
    'def calculate_total(items: list, tax: float) -> Decimal': 42
    'async def fetch_data(url: str) -> dict': 67
  myapp/helpers.py:
    'def validate_email(email: str) -> bool': 15
```

### `class` — List Classes

Extract all class definitions with fields, methods, and inheritance.

```bash
pyr class [PATTERN...] [--public | --private]
```

**Example:**
```bash
$ pyr -t myapp/ class
```
```yaml
files:
  myapp/models.py:
    'class UserService(BaseService)':
      fields:
        'name: str': 24
        'email: str': 25
      methods:
        'def create_user(self, data: UserCreate) -> User': 45
        'def get_by_id(self, user_id: int) -> User | None': 67
```

### `enum` — List Enums

Extract all enum definitions (classes inheriting from `Enum`, `IntEnum`, `StrEnum`, etc.).

```bash
pyr enum [PATTERN...]
```

**Example:**
```bash
$ pyr -t myapp/ enum
```
```yaml
files:
  myapp/types.py:
    'class Status(IntEnum)': 9
    'class Color(Enum)': 4
    'class Direction(StrEnum)': 15
```

### `module` — Show Module Structure

Display the package/module hierarchy.

```bash
pyr module [PATTERN...]
```

**Example:**
```bash
$ pyr -t myapp/ module
```
```yaml
modules:
  __init__.py:
    type: module
  models.py:
    type: module
  services:
    type: package
    children:
      services/__init__.py:
        type: module
      services/user.py:
        type: module
```

### `dump` — Comprehensive Output

Combines functions, classes (flattened as `ClassName.method`), and enums.

```bash
pyr dump [PATTERN...]
```

## Pattern Matching

All subcommands accept optional patterns that filter results by name. Patterns use **cascading match logic**:

1. **Starts with** (case-sensitive) — highest priority
2. **Starts with** (case-insensitive)
3. **Contains** (case-sensitive)
4. **Contains** (case-insensitive) — lowest priority

The first level that produces matches wins. This ensures precise matches are preferred over fuzzy ones.

**Examples:**
```bash
# Find functions starting with "test"
pyr function test

# Multiple patterns (OR logic)
pyr function compute validate

# Pattern matching is smart
pyr function Test      # Matches: test_foo (case-insensitive startswith)
pyr function serv      # Matches: UserService (contains)
```

**Important:** Pattern matching applies to the *name*, not the full signature. For functions, it matches the function name (not `def` or `async def`). For classes, it matches the class name (not `class` or base classes).

## Visibility Filtering

Filter functions and class members by Python's underscore convention:

```bash
# Only public (names NOT starting with _)
pyr function --public
pyr class --public

# Only private (names starting with _)
pyr function --private
pyr class --private
```

For `class`, visibility filtering applies to both fields and methods within each class.

## Global Options

| Option | Short | Description |
|--------|-------|-------------|
| `--target <PATH>` | `-t` | Files or directories to analyze (default: `.`) |
| `--json` | `-j` | Force JSON output (default: YAML, or JSON when piped) |
| `--alphabetical` | `-a` | Sort symbols alphabetically (default: file order) |
| `--help` | `-h` | Show help |
| `--version` | `-V` | Show version |

**Multiple targets:**
```bash
pyr -t src/ -t tests/ -t scripts/ function
```

## Output Formats

### YAML (Default for TTY)

Human-readable, great for interactive use:

```yaml
files:
  src/utils.py:
    'def helper(x: int) -> str': 10
```

### JSON (Default for Pipes)

Machine-readable, ideal for scripting and LLM consumption:

```json
{
  "files": {
    "src/utils.py": {
      "def helper(x: int) -> str": 10
    }
  }
}
```

Force JSON output: `pyr --json function`

## Real-World Examples

### Find All Test Functions
```bash
pyr -t tests/ function test
```

### List Public API of a Module
```bash
pyr -t myapp/api.py function --public
```

### Explore Class Hierarchy
```bash
pyr -t myapp/models/ class Service
```

### Get Module Structure for Documentation
```bash
pyr -t src/ module --json | jq '.modules'
```

### Feed to an LLM Agent
```bash
# Pipe structured output to your LLM tool
pyr dump | llm-tool analyze-codebase
```

## Architecture

```
src/
├── main.rs          # Entry point, CLI dispatch
├── cli.rs           # Clap argument definitions
├── parser.rs        # rustpython-parser integration
├── pattern.rs       # Pattern matching logic
├── walk.rs          # File discovery, parallel iteration
├── analysis/
│   ├── functions.rs # Function extraction
│   ├── classes.rs   # Class/method extraction
│   ├── enums.rs     # Enum extraction
│   └── modules.rs   # Module tree building
└── output/
    ├── types.rs     # Output structs (serde)
    └── format.rs    # YAML/JSON formatting
```

## Design Principles

1. **Fast** — Parallel parsing, no unnecessary work
2. **Deterministic** — Same input = same output (stable ordering)
3. **Clean Output** — Structured data to stdout, diagnostics to stderr
4. **Simple CLI** — Obvious defaults, minimal required flags
5. **Agent-Friendly** — Output optimized for LLM token efficiency

## File Discovery

- Recursively finds `*.py` files in directories
- Respects common ignores: `__pycache__`, `.git`, `venv`, `.venv`, `node_modules`, `.tox`, `.pytest_cache`, `.mypy_cache`, `.ruff_cache`, `dist`, `build`, `*.egg-info`
- Files are sorted alphabetically for deterministic output

## Limitations

- **Top-level only** — Nested functions/classes not extracted
- **No import resolution** — Enum detection is best-effort based on base class name
- **No docstrings** — Only signatures extracted
- **No call graph** — Usage/callsite analysis not implemented

## Contributing

```bash
# Run tests
cargo test

# Run CI checks
otto ci

# Format code
cargo fmt

# Lint
cargo clippy
```

## License

MIT

---

*Built with 🦀 Rust for 🤖 AI agents*

