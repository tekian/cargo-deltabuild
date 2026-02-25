# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.1] - 2026-02-25

### Added

- Add filters to standard error output (#10)
- Add a baseline set of unit tests (#5)
- Add rich CI functionality (build, clippy, spell check)
- Integrate cargo-make for build orchestration (#11)

### Changed

- Renamed project to cargo-delta (#9)
- Move crates to `crates/` workspace layout (#11)
- Make mutation analysis optional (#11)
- Improve command-line processing with clap (#7)
- Split into bin+lib to enable integration testing (#8)

### Fixed

- Attempt to determine git origin base branch automatically (#3)
- Fix build, clippy, and spell check warnings

### Dependencies

- Bump taiki-e/upload-rust-binary-action from 1.27.0 to 1.28.0 (#6)

[0.2.1]: https://github.com/tekian/cargo-delta/compare/0.1...HEAD
