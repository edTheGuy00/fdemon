# Contributing to Flutter Demon

Thank you for your interest in contributing to Flutter Demon! We welcome contributions from the community.

## License and Copyright

Flutter Demon is licensed under the [Business Source License 1.1 (BSL-1.1)](LICENSE), which converts to AGPL-3.0 after four years from each release.

### Important Information for Contributors

By contributing to this project, you agree that:

1. **Your contributions will be licensed under BSL-1.1** - All contributions become part of the Flutter Demon codebase under the same BSL-1.1 license.

2. **Copyright retention** - You retain copyright to your contributions, but grant the project maintainers a perpetual, worldwide, non-exclusive, royalty-free license to use, modify, and distribute your contributions.

3. **Contributor License Agreement (CLA)** - For significant contributions, you may be asked to sign a CLA. This protects both you and the project.

4. **Commercial use** - Your contributions may be used in both the open-source version and any commercial offerings (e.g., pro features).

5. **Future license changes** - When the license converts to AGPL-3.0 after 4 years, your contributions will also be under AGPL-3.0.

### What This Means

- ✅ You can contribute freely
- ✅ Your work will be credited
- ✅ You retain copyright to your contributions
- ✅ Your code remains open source (BSL → AGPL)
- ⚠️ The maintainer can use your contributions in commercial pro features
- ⚠️ You grant perpetual rights to use your code

If you have concerns about these terms, please open an issue to discuss before contributing.

## How to Contribute

### Prerequisites

- **Rust** 1.70 or later
- **Flutter SDK** in your PATH
- A terminal with Unicode support
- Git

### Development Setup

```bash
# Clone the repository
git clone https://github.com/edTheGuy00/fdemon.git
cd flutter-demon

# Build the project
cargo build

# Run tests
cargo test

# Run the binary
cargo run -- /path/to/flutter/project
```

### Code Quality

Before submitting a PR, ensure your code passes all checks:

```bash
# Format code
cargo fmt

# Run lints
cargo clippy

# Run all tests
cargo test

# Run specific tests
cargo test log_view
```

### Architecture

Flutter Demon follows **The Elm Architecture (TEA)** pattern. Please read:
- [ARCHITECTURE.md](docs/ARCHITECTURE.md) - System design and patterns
- [CLAUDE.md](CLAUDE.md) - Project overview and build commands

Key principles:
- **Separation of concerns** - TUI, daemon, core are separate modules
- **Immutable state** - State transitions through `Message` → `update()` → `new_state`
- **Testability** - Unit tests for handlers, integration tests for features
- **Documentation** - Document public APIs and complex logic

### Contribution Workflow

1. **Fork the repository** on GitHub
2. **Create a feature branch** from `main`
   ```bash
   git checkout -b feature/my-awesome-feature
   ```
3. **Make your changes** following the code style
4. **Write tests** for new functionality
5. **Update documentation** if needed
6. **Run all quality checks** (fmt, clippy, test)
7. **Commit with clear messages**
   ```bash
   git commit -m "feat: add awesome feature"
   ```
8. **Push to your fork**
   ```bash
   git push origin feature/my-awesome-feature
   ```
9. **Open a Pull Request** with a clear description

### Commit Message Convention

We follow [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add new feature
fix: bug fix
docs: documentation changes
test: add or update tests
refactor: code refactoring
style: formatting changes
chore: maintenance tasks
```

Examples:
```
feat: add hot restart support
fix: resolve crash on empty log buffer
docs: update KEYBINDINGS.md
test: add tests for session manager
refactor: extract log filtering to separate module
```

### Pull Request Guidelines

**Good PR:**
- ✅ Clear title and description
- ✅ References related issues (`Fixes #123`)
- ✅ Includes tests
- ✅ Updates documentation
- ✅ Passes all CI checks
- ✅ Single, focused change

**Please avoid:**
- ❌ Multiple unrelated changes in one PR
- ❌ Reformatting large portions of code
- ❌ Breaking changes without discussion
- ❌ Commits with "WIP" or "tmp" messages
- ❌ Large PRs (>500 lines) without prior discussion

### Testing

Tests are crucial! Please include:

**Unit tests** - For business logic
```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature() {
        // Your test here
    }
}
```

**Integration tests** - For end-to-end features
```rust
// tests/integration_test.rs
#[tokio::test]
async fn test_integration() {
    // Your test here
}
```

**Snapshot tests** - For TUI rendering (using `insta`)
```rust
#[test]
fn test_widget_rendering() {
    insta::assert_snapshot!(rendered_output);
}
```

### Documentation

Update documentation when you:
- Add new features → Update README.md
- Change keybindings → Update docs/KEYBINDINGS.md
- Add configuration → Update docs/CONFIGURATION.md
- Change architecture → Update docs/ARCHITECTURE.md

### Code Review Process

1. Maintainers will review your PR
2. Address feedback and update your PR
3. Once approved, your PR will be merged
4. Your contribution will be credited in release notes

## Types of Contributions

### 🐛 Bug Reports

Found a bug? Please open an issue with:
- Clear title describing the bug
- Steps to reproduce
- Expected vs. actual behavior
- Environment (OS, Flutter version, terminal)
- Logs or screenshots if applicable

### 💡 Feature Requests

Have an idea? Open an issue with:
- Clear description of the feature
- Use case and benefits
- Potential implementation approach (optional)

### 📚 Documentation

Documentation improvements are always welcome:
- Fix typos
- Clarify confusing sections
- Add examples
- Improve architecture docs

### 🎨 UI/UX Improvements

Terminal UI enhancements:
- Better layouts
- Improved colors/themes
- More intuitive keybindings
- Accessibility improvements

### 🧪 Testing

Help improve test coverage:
- Add missing unit tests
- Create integration tests
- Add snapshot tests for UI components

## Code of Conduct

### Our Standards

- ✅ Be respectful and inclusive
- ✅ Welcome newcomers
- ✅ Accept constructive criticism
- ✅ Focus on what's best for the community
- ❌ No harassment or discriminatory language
- ❌ No trolling or insulting comments

### Enforcement

Unacceptable behavior will result in:
1. Warning
2. Temporary ban
3. Permanent ban

Report issues to the maintainers.

## Recognition

Contributors will be:
- Listed in release notes
- Credited in commit history
- Acknowledged in documentation (for major contributions)

## Questions?

- 💬 Open a [Discussion](https://github.com/edTheGuy00/fdemon/discussions)
- 🐛 Report bugs via [Issues](https://github.com/edTheGuy00/fdemon/issues)
- 📧 Contact maintainers (see README.md)

## Resources

- [README.md](README.md) - Project overview
- [ARCHITECTURE.md](docs/ARCHITECTURE.md) - System design
- [KEYBINDINGS.md](docs/KEYBINDINGS.md) - Keyboard controls
- [CONFIGURATION.md](docs/CONFIGURATION.md) - Configuration guide
- [CLAUDE.md](CLAUDE.md) - Development workflow
- [LICENSE](LICENSE) - BSL-1.1 license terms

---

Thank you for contributing to Flutter Demon! 🔥😈