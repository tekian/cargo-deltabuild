# cargo-deltabuild

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A **best-effort** tool designed to identify which crates in a Cargo workspace are impacted by changes in a Git feature branch. By analyzing dependencies and detecting affected crates, this tool helps optimize CI/CD pipelines. It enables targeted builds, tests, benchmarks, or mutations on a smaller subset of crates, reducing build times in large projects.

## Features

- 🔍 **Dependency Analysis**: Analyses Rust workspace and builds dependency tree.
- 📊 **Git Integration**: Compares changes against a baseline branch to determine affected files.
- 🎯 **Crate Impact**: Identifies which crates are affected by file changes using the tree.
- ⚙️ **Configurable**: Supports custom configuration for different parsing strategies

## Installation

```bash
cargo install cargo-deltabuild
```

Or build from source:

```bash
git clone https://github.com/tekian/cargo-deltabuild
cd cargo-deltabuild
cargo build --release
```

## Usage

The tool operates in two phases:

### 1. Analyze Phase

First, analyze your workspace to create a dependency tree:

```bash
cargo deltabuild analyze > analysis.json
```

This command:
- Scans your Cargo workspace
- Analyzes Rust source files for dependencies
- Outputs a JSON file containing the complete dependency graph

### 2. Run Phase

Compare two analysis files to determine affected crates:

```bash
cargo deltabuild run --baseline baseline.json --current current.json
```

This command:
- Compares the current branch against a baseline
- Uses git to detect changed/deleted files
- Outputs affected crates as JSON

## Typical CI/CD Workflow

```bash
# On your main branch
git checkout main
cargo deltabuild analyze > main.json

# On your feature branch
git checkout feature-branch
cargo deltabuild analyze > feature.json

# Find affected crates
cargo deltabuild run --baseline main.json --current feature.json
```

## Configuration

Create a `config.toml` file to customize the analysis:

```toml
[parser]
# Enable/disable file reference detection from method calls
file_refs = true
file_methods = ["file", "from_file", "load", "open", "read"]

# Enable/disable detection of include macros
includes = true
include_macros = ["include_str", "include_bytes"]

# Enable/disable following mod declarations
mods = true

# Enable/disable pattern-based assumptions
assume = true
assume_patterns = ["*.proto", "*.snap"]

[git]
# Remote branch to compare against
remote_branch = "origin/main"

[files]
# Patterns to exclude from analysis
exclude_patterns = ["target/**", "*.tmp"]
```

## Detection Methods

The tool uses several heuristics to detect file dependencies:

1. **Module Dependencies**: Follows `mod` declarations and `#[path]` attributes
2. **Include Macros**: Detects `include_str!()` and `include_bytes!()` macros
3. **File References**: Identifies method calls that load files (e.g., `::file()`, `::from_file()`)
4. **Pattern Matching**: Assumes certain file types are dependencies (e.g., `.proto`, `.snap`)

## Output Format

The tool outputs JSON with affected crates:

```json
{
  "AffectedCrates": [
    "my-crate",
    "dependent-crate",
    "another-crate"
  ]
}
```

## Limitations

This tool is **best-effort** and may not detect all dependencies:

- Dynamic file paths computed at runtime
- Conditional compilation dependencies
- Other dependencies not captured by the heuristics


## Example

```bash
# Analyze current workspace
$ cargo deltabuild analyze > feature.json
Analyzing workspace..

Found 15 crate(s) in the workspace.
Found 247 file(s) in the workspace.

Analysis finished in 1.23s

# Compare with baseline
$ cargo deltabuild run --baseline main.json --current feature.json
Running deltabuild..

Looking up git changes..

Changed file: "src/api/mod.rs"
Changed file: "src/utils.rs"

Using baseline analysis   : main.json
Using current analysis    : feature.json

{
  "AffectedCrates": [
    "my-api",
    "my-utils",
    "my-app"
  ]
}

Impacts 3 out of 15 crates (20.0%)
```

## Contributing

Contributions are welcome! Please feel free to fork the repository and submit a pull request.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.