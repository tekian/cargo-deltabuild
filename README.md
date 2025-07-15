# cargo-deltabuild

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

`cargo-deltabuild` detects which crates in a Cargo workspace are affected by changes in a Git feature branch. Build, test, and benchmarks only the crates you need—saving time and resources in your CI/CD pipeline.

## Features

- **Static Detection**: Analyzes the full dependency graph, following Rust modules, includes, and patterns.
- **Runtime Detection**: Detects dynamically loaded files using common method signatures and custom patterns.
- **Impact Categorization**: Separates crates into _Modified_, _Affected_, and _Required_ for precise CI/CD targeting.
- **Configurability**: Highly customizable via `config.toml`, with per-crate overrides for parsing and detection.
- **Dual-branch Git Detection**: Compares two branches or commits to find both modified and deleted files.

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

You can customize `cargo-deltabuild` by providing a `config.toml` file. Pass it to either subcommand with the `-c` or `--config` option:

```bash
cargo deltabuild analyze -c config.toml 
cargo deltabuild run -c config.toml
```

Configuration options can be set globally and overridden per crate. For example, you can enable a feature for all crates, but disable or adjust it for a specific crate in the config file:

```toml
[parser]
foo = true
foo_patterns = ["*.foo", "*.bar"]

[parser.my-crate]
foo = false  # Override for a specific crate
foo_patterns = ["*.baz"]
```

Default settings are provided in [`config.toml.example`](./config.toml.example).

## Detection Methods

The tool uses several heuristics to detect file dependencies:

1. **Module Dependencies**: Follows `mod` declarations and `#[path]` attributes
2. **Include Macros**: Detects `include_str!()` and `include_bytes!()` macros
3. **File References**: Identifies method calls that load files (e.g., `::file()`, `::from_file()`)
4. **Pattern Matching**: Assumes certain file types are dependencies (e.g., `.proto`, `.snap`)
5. **Trip Wire**: Certain critical files trigger a full workspace rebuild when changed

## Output Format

The tool outputs JSON with three categories of affected crates:

- **Modified**: Crates directly modified by Git changes
- **Affected**: Modified crates plus all their dependents (direct and indirect)
- **Required**: All crates needed - affected crates plus all their dependencies

## Limitations

This tool is **best-effort** and may not detect all dependencies:

- Dynamic file paths computed at runtime
- Conditional compilation dependencies
- Other dependencies not captured by the heuristics

**Note**: Use trip wire patterns for critical files that should trigger full rebuilds when changed (e.g., workspace configuration, build scripts).


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
Required      4 (Affected crates plus all their dependencies.)
Total        15 (Total crates in this workspace.)
```

## Contributing

Contributions are welcome! Please feel free to fork the repository and submit a pull request.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.