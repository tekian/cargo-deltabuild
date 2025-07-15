# cargo-deltabuild

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

`cargo-deltabuild` detects which crates in a Cargo workspace are affected by changes in a Git feature branch. Build, test, and benchmarks only the crates you need—saving time and resources in your CI/CD pipeline.

- **Robust Detection**: Uses code analysis, pattern matching and runtime heuristics to identify dependencies.
- **Impact Categorization**: Separates crates into _Modified_, _Affected_, and _Required_ for precise targeting.
- **Configurability**: Highly customizable via config, with per-crate overrides for parsing and detection.
- **Dual-branch Git Detection**: Compares two branches or commits to find both modified and deleted files.
- **File Control Mechanisms**: Exclude files from analysis or trigger a full rebuild when critical files change.

## Installation

```bash
cargo install cargo-deltabuild
```

## Usage

1. **Check out the baseline branch and analyze:**
   ```bash
   git checkout main
   cargo deltabuild analyze > main.json
   ```

2. **Check out your feature branch and analyze:**
   ```bash
   git checkout feature-branch
   cargo deltabuild analyze > feature.json
   ```

3. **Compare analyses to find impacted crates:**
   ```bash
   cargo deltabuild run --baseline main.json --current feature.json
   ```

## Configuration

You can customize `cargo-deltabuild` by providing a `-c config.toml` argument to the command.

```bash
cargo deltabuild analyze -c config.toml # ...
cargo deltabuild run -c config.toml # ...
```

Configuration options can be set globally and overridden per crate. For example, you can enable a feature for all crates, but disable or adjust it for a specific crate in the config file:

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

Config example (default is empty):

```toml
[parser]
mod_macros = ["my_mod"]
```

### Include Macros

Detects files included via macros such as `include_str!` and `include_bytes!`, assuming the first argument is the name of the file.

Config default:

```toml
[parser]
include_macros = ["include_str", "include_bytes"]
```

### Pattern-based Assumptions

Assumes certain files are dependencies based on glob patterns (e.g., `*.proto`, `*.snap`).

Config default:

```toml
[parser]
assume = true
assume_patterns = ["*.proto", "*.snap"]
```

### File Method Matching

Detects files loaded at runtime by matching method names (e.g., `from_file`, `load`, `open`), assuming the first argument is the name of the file.

Config default:

```toml
[parser]
file_refs = true
file_methods = ["file", "from_file", "load", "open", "read", "load_from"]
```

## File Control Mechanisms

### File Exclusion

Exclude files and folders from analysis using glob patterns. Useful for ignoring build artifacts, temp files, etc.

Config default:

```toml
file_exclude_patterns = ["target/**", "*.tmp"]
```

### Trip Wire

If any changed or deleted file matches a configured trip wire pattern, all crates are considered impacted. Use this for critical files like `Cargo.toml`, build scripts, or configuration files.

Config example:

```toml
trip_wire_patterns = [
    "Cargo.toml",       # top-level Cargo.toml
    "deltabuild.toml"   # DeltaBuild config file
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
$ cargo deltabuild run --baseline main.json --current feature.json
Running deltabuild..

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