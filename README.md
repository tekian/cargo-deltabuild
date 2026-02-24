# cargo-delta

[![crate.io](https://img.shields.io/crates/v/cargo-delta.svg)](https://crates.io/crates/cargo-delta)
[![CI](https://github.com/tekian/cargo-delta/workflows/main/badge.svg)](https://github.com/tekian/cargo-delta/actions)
[![Coverage](https://codecov.io/gh/tekian/cargo-delta/graph/badge.svg)](https://codecov.io/gh/tekian/cargo-delta)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](./LICENSE)

`cargo-delta` detects which crates in a Cargo workspace are impacted by changes in a Git feature branch. Build, test, and benchmark only the crates you need, saving time and resources in your CI/CD pipeline.

- **Robust Detection**: Uses code analysis, pattern matching and runtime heuristics to identify dependencies.
- **Impact Categorization**: Separates crates into _Modified_, _Affected_, and _Required_ for precise targeting.
- **Configurability**: Highly customizable via config, with per-crate overrides for parsing and detection.
- **Dual-branch Git Detection**: Compares two branches or commits to find both modified and deleted files.
- **File Control Mechanisms**: Exclude files from analysis or trigger a full rebuild when critical files change.

## Installation

```bash
cargo install cargo-delta
```

## Usage

1. **Check out the baseline branch and analyze:**
   ```bash
   git checkout main
   cargo delta analyze > main.json
   ```

2. **Check out your feature branch and analyze:**
   ```bash
   git checkout feature-branch
   cargo delta analyze > feature.json
   ```

3. **Compare analyses to find impacted crates:**
   ```bash
   cargo delta run --baseline main.json --current feature.json
   ```

## Configuration

You can customize `cargo-delta` by providing a `-c config.toml` argument to the command.

```bash
cargo delta analyze -c config.toml # ...
cargo delta run -c config.toml # ...
```

Configuration options can be set globally and overridden per crate. For example:

```toml
[parser]
foo = true
foo_patterns = ["*.foo", "*.bar"]

[parser.my-crate]
foo_patterns = ["*.baz"] # Override for a specific crate
```

Default settings are provided in [`config.toml.example`](./config.toml.example).

## Robust Detection

### Module Traversal

Follows `mod` declarations and `#[path]` attributes to discover all Rust modules in the workspace.

### Mod Macros

Discovers modules declared via custom macros (e.g., `my_mod!`), assuming first argument is the name of the module.

Config default:

```toml
[parser]
mod_macros = []
```

Config example:

```toml
[parser]
mod_macros = ["my_mod"]  # my_mod!(foo)
```

### Include Macros

Detects files included via macros such as `include_str!` and `include_bytes!`, assuming the first argument is the name of the file.

Config default:

```toml
[parser]
includes = true
include_macros = [
    "include_str",   # include_str!("file.txt")
    "include_bytes"  # include_bytes!("file.bin")
]
```

### Pattern-based Assumptions

Assumes certain files are dependencies based on glob patterns (e.g., `*.proto`, `*.snap`).

Config default:

```toml
[parser]
assume = false
assume_patterns = []
```

Config example:

```toml
[parser.grpc_crate]
assume = true
assume_patterns = [".proto"]
```

### File Method Matching

Detects files loaded at runtime by matching method names (e.g., `from_file`, `load`, `open`), assuming the first argument is the name of the file.

Config default:

```toml
[parser]
file_refs = true
file_methods = [
    "file",       # ::file(path, ...)
    "from_file",  # ::from_file(path, ...)
    "load",       # ::load(path, ...)
    "open",       # ::open(path, ...)
    "read",       # ::read(path, ...)
    "load_from"   # ::load_from(path, ...)
]
```

## File Control Mechanisms

### File Exclusion

Exclude files and folders from analysis using glob patterns. Useful for ignoring build artifacts, temp files, etc.

Config default:

```toml
file_exclude_patterns = ["target/**", "*.tmp"]
```

### Trip Wire

If any changed or deleted file matches a configured trip wire pattern, all crates are considered impacted. Use this for critical files like top-level `Cargo.toml`, build scripts, or configuration files.

Config default:

```toml
trip_wire_patterns = []
```


Config example:

```toml
trip_wire_patterns = [
    "Cargo.toml",       # top-level Cargo.toml
    "delta.toml"   # Delta config file
]
```

## Understanding Output

### Analyze

Analyze phase produces JSON file that's intended to be consumed by `run` phase.

- **files**: Nested tree of file dependencies as detected by all the heuristics.
- **crates**: Dependency relationships between crates within the workspace.

### Run

Run phase produces JSON file that's intended to be consumed by _your_ CI/CD.

- **Modified**: Crates directly modified by Git changes. 
- **Affected**: Modified crates plus all their dependents, direct and indirect.
- **Required**: Affected crates plus all their dependencies, direct and indirect.


## Limitations

This tool is **best-effort** and may not detect all dependencies:

- Dynamic file paths computed at runtime
- Conditional compilation dependencies
- Other dependencies not captured by the heuristics


## Example

```bash
$ cargo delta run --baseline main.json --current feature.json
Running delta..

Looking up git changes..

Changed file: "src/api/mod.rs"
Changed file: "src/utils.rs"

Using baseline analysis : main.json
Using current analysis  : feature.json

{
  "Modified": [
    "my-api",
    "my-utils"
  ],
  "Affected": [
    "my-api",
    "my-utils",
    "my-app"
  ],
  "Required": [
    "my-api",
    "my-utils", 
    "my-app",
    "common-lib"
  ]
}

Modified      2 (Crates directly modified by Git changes.)
Affected      3 (Modified crates plus all their dependents, direct and indirect.)
Required      4 (Affected crates plus all their dependencies, direct and indirect.)
Total        15 (Total crates in this workspace.)
```

## Contributing

Contributions are welcome! Please feel free to fork the repository and submit a pull request.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.