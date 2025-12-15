# pyr design document

a rust cli tool for python codebase analysis, designed for agentic llm consumption.

## purpose

provide fast, structured queries about python codebases so llm agents can understand project structure without reading every file. answers questions like:

- what functions exist and where?
- what classes exist, what methods do they have?
- what's the module structure?

## parser choice

**crate:** `rustpython-parser`

rationale:
- pure rust, no python runtime required
- available on crates.io (stable versioning)
- used by ~52 crates including analysis tools
- sufficient performance for project-scale analysis

rejected alternatives:
- `python-ast`: requires python runtime via pyo3, experimental, goal is transpilation
- `ruff_python_parser`: not on crates.io, git dependency complicates builds

## cli design

```
pyr [OPTIONS] [TARGETS]...

TARGETS:
    one or more files or directories to analyze
    default: current working directory

OPTIONS:
    -j, --json           force json output (default: yaml)
    -a, --alphabetical   sort symbols alphabetically (default: file order by line)
    -h, --help           print help
    -V, --version        print version
```

### subcommands

```
pyr functions [TARGETS]...    list all functions with signatures and locations
pyr classes [TARGETS]...      list all classes with methods and inheritance
pyr enums [TARGETS]...        list all enum definitions
pyr modules [TARGETS]...      show module/package structure
pyr dump [TARGETS]...         comprehensive output (all of the above)
```

## output format

### default: yaml

yaml output by default, but restricted to json-compatible subset:
- no anchors/aliases
- no multi-line strings with `|` or `>`
- no custom tags

### json mode

triggered by:
- `--json` flag
- stdout is not a tty (pipeline detection)

### structure examples

output is nested by file. each subcommand filters to relevant content.

**functions output (`pyr functions`):**
```yaml
files:
  src/billing.py:
    functions:
      calculate_total:
        line: 42
        signature: "def calculate_total(items: list[Item], tax_rate: float = 0.0) -> Decimal"
  src/utils.py:
    functions:
      validate_email:
        line: 15
        signature: "def validate_email(email: str) -> bool"
```

**classes output (`pyr classes`):**
```yaml
files:
  src/services/user.py:
    classes:
      UserService:
        line: 23
        bases:
          - BaseService
          - Auditable
        methods:
          create_user:
            line: 45
            signature: "def create_user(self, data: UserCreate) -> User"
          get_by_id:
            line: 67
            signature: "def get_by_id(self, user_id: int) -> User | None"
```

**enums output (`pyr enums`):**
```yaml
files:
  src/models.py:
    enums:
      OrderStatus:
        line: 12
        members:
          - PENDING
          - PROCESSING
          - SHIPPED
          - DELIVERED
```

**dump output (`pyr dump`):**
```yaml
files:
  src/billing.py:
    functions:
      calculate_total:
        line: 42
        signature: "def calculate_total(items: list[Item], tax_rate: float = 0.0) -> Decimal"
  src/models.py:
    enums:
      OrderStatus:
        line: 12
        members:
          - PENDING
          - PROCESSING
          - SHIPPED
          - DELIVERED
  src/services/user.py:
    classes:
      UserService:
        line: 23
        bases:
          - BaseService
          - Auditable
        methods:
          create_user:
            line: 45
            signature: "def create_user(self, data: UserCreate) -> User"
          get_by_id:
            line: 67
            signature: "def get_by_id(self, user_id: int) -> User | None"
```

**modules output (`pyr modules`):**
```yaml
modules:
  src:
    type: package
    children:
      src/models.py:
        type: module
      src/services:
        type: package
        children:
          src/services/user.py:
            type: module
```

## architecture

```
src/
├── main.rs          # entry point, cli dispatch
├── cli.rs           # clap argument definitions
├── parser.rs        # rustpython-parser integration
├── analysis/
│   ├── mod.rs
│   ├── functions.rs # function extraction
│   ├── classes.rs   # class/method extraction
│   ├── enums.rs     # enum extraction
│   └── modules.rs   # module tree building
├── output/
│   ├── mod.rs
│   ├── types.rs     # output structs (serde)
│   └── format.rs    # yaml/json formatting, tty detection
└── walk.rs          # file discovery, parallel iteration
```

## dependencies

```toml
[dependencies]
clap = { version = "4", features = ["derive"] }
rustpython-parser = "0.4"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
rayon = "1"
eyre = "0.6"
walkdir = "2"
# note: use std::io::IsTerminal (stable since rust 1.70) instead of atty crate
```

## error handling

- use `eyre` for error propagation
- syntax errors in python files: skip file, warn to stderr
- missing files/directories: error, exit non-zero
- stdout remains clean (only analysis output)
- stderr for warnings and errors

## file discovery

1. accept one or more targets (files or directories)
2. for directories: recursively find `*.py` files
3. respect common ignores: `__pycache__`, `.git`, `venv`, `.venv`, `node_modules`
4. use `walkdir` for traversal
5. parallelize parsing with `rayon`

## scope decisions

### in scope (v1)
- top-level functions
- classes and their methods
- enums (classes inheriting from `Enum`)
- module structure
- function/method signatures with type hints
- file:line locations

### out of scope (v1)
- callsite/usage analysis
- decorator handling
- docstring extraction
- import analysis
- nested functions/classes
- caching

### future consideration
- `pyr usages <symbol>` - find callsites
- `pyr find <symbol>` - find definitions
- `pyr imports` - import graph
- incremental analysis with caching

## behavior details

### enum detection

best-effort based on base class name. a class is treated as an enum if any of its
base classes include the string `Enum` (e.g., `Enum`, `IntEnum`, `StrEnum`).
no import resolution—if someone does `from enum import Enum as E`, we won't catch it.

### sorting

- **default**: symbols appear in file order (by line number)
- **`--alphabetical`**: sorts symbols by name within each file
- files are always sorted alphabetically by path

### empty results

if no symbols found, output empty structure:
```yaml
files: {}
```

## design principles

1. **fast**: parallel parsing, no unnecessary work
2. **deterministic**: same input = same output (stable ordering)
3. **clean output**: structured data to stdout, diagnostics to stderr
4. **simple cli**: obvious defaults, minimal required flags
5. **agent-friendly**: output optimized for llm token efficiency

