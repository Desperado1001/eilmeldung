# Repository Guidelines

## Project Structure & Module Organization
`eilmeldung` is a Rust TUI application.
- `src/main.rs`: entry point.
- `src/ui/`: UI state and rendering (`articles_list`, `feeds_list`, `article_content`, popups).
- `src/config/`: configuration models and parsing.
- `src/query/`, `src/messages/`, `src/input/`: query language, app events/commands, key handling.
- `docs/`: user and contributor-facing documentation.
- `examples/`: sample TOML configuration files shipped with releases.
- `assets/`: static images used in docs/UI.
- `.github/workflows/`: CI definitions for lint, test, and release builds.

## Build, Test, and Development Commands
- `cargo run`: start the app in development mode.
- `cargo build`: compile debug binary.
- `cargo build --release`: compile optimized binary.
- `cargo test --all-features`: run all tests (matches CI).
- `cargo fmt --check`: verify formatting.
- `cargo clippy --all-targets --all-features -- -D warnings`: enforce lint-clean code (matches CI policy).

## Coding Style & Naming Conventions
- Follow `rustfmt` defaults (4-space indentation; no manual alignment hacks).
- Keep modules focused and small; prefer feature-oriented folders under `src/ui/` and `src/config/`.
- Use `snake_case` for files, modules, functions, and variables; `PascalCase` for structs/enums/traits.
- Prefer explicit names over abbreviations (for example `login_configuration.rs` over short aliases).

## Testing Guidelines
- Place unit tests near implementation using `#[cfg(test)] mod test`.
- Use `#[test]` for simple checks and `rstest` for parameterized cases.
- Name tests by behavior, e.g. `parses_relative_date_query` or `rejects_empty_share_target`.
- Run `cargo test --all-features` before opening a PR.

## Commit & Pull Request Guidelines
- Use concise, imperative commit subjects; optional prefixes like `chore:` are common.
- Reference issues/PRs when relevant, e.g. `fix query parsing for tags (#144)`.
- Keep commits scoped to one logical change.
- PRs should include:
  - clear problem/solution summary,
  - testing notes (commands run),
  - screenshots or terminal captures for UI/UX-visible changes,
  - linked issue when applicable.

## Security & Configuration Tips
- Never commit secrets or local machine paths in config examples.
- Keep `examples/default-config.toml` and related docs updated when adding config fields or commands.
