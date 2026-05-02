# Contributing to filo

Thanks for your interest in contributing. This guide helps you get from first change to merged PR quickly.

## Ground Rules

- Keep changes focused. Small, scoped PRs are reviewed faster.
- Add or update tests for behavior changes.
- Update docs when CLI behavior or config shape changes.
- Be respectful and constructive in all project interactions.

## Local Setup

1. Fork and clone the repository.
2. Install Rust stable (`rustup toolchain install stable`).
3. Run checks once before editing:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## Project Structure (Quick Map)

- `src/main.rs`: thin CLI entry point
- `src/lib.rs`: core library API
- `src/commands/`: one module per command
- `src/config/`: config schema + defaults
- `src/organizer/`: planning + move/rename/duplicate logic
- `src/watcher/`: filesystem watching and debounce logic
- `tests/integration.rs`: end-to-end integration tests

## Development Workflow

1. Create a branch from `develop`.
2. Make changes in small commits with clear messages.
3. Run all checks locally:

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

4. Open a PR to `develop` and include:
- Problem statement
- What changed
- Why this approach
- Any migration or behavior notes

## Branching and Releases

- `develop`: integration branch for day-to-day feature work and beta validation.
- `main`: stable release branch only.
- Feature branches should branch from and merge back into `develop`.
- Promote to stable by opening a PR from `develop` to `main` after beta validation passes.

## Versioning

`filo` follows SemVer and uses the package version in `Cargo.toml` as the source of truth.

- `Cargo.toml` `[package].version` is the crate/app version.
- `filo --version` comes from that same Cargo package version.
- Release tags should match the package version with a leading `v`.

Examples:

- `Cargo.toml` version `0.2.0-beta.1` -> tag `v0.2.0-beta.1` (beta/prerelease)
- `Cargo.toml` version `0.2.0` -> tag `v0.2.0` (stable)

Suggested release flow:

1. On `develop`, bump to next beta version and merge.
2. Tag that commit (`vX.Y.Z-beta.N`) and push the tag. GitHub will publish a prerelease.
3. After beta signoff, promote `develop` -> `main`.
4. Bump/finalize version to `X.Y.Z` (without prerelease suffix) if needed.
5. Tag `main` with `vX.Y.Z` and push the tag. GitHub will publish a stable release.

## Pull Request Checklist

- [ ] Code is formatted (`cargo fmt --check`)
- [ ] Lints pass (`cargo clippy --all-targets -- -D warnings`)
- [ ] Tests pass (`cargo test`)
- [ ] Tests added/updated for behavior changes
- [ ] Docs/README updated if needed
- [ ] PR description explains user-facing impact

## Reporting Bugs

Please use the bug issue template and include:

- Exact command run
- Expected behavior vs actual behavior
- OS + shell + Rust version
- Minimal reproduction steps

## Security Issues

Please do not open public issues for security vulnerabilities.
See [`SECURITY.md`](SECURITY.md) for responsible disclosure steps.

## Code of Conduct

This project follows the guidelines in [`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md).
