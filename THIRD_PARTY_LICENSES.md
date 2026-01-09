# Third-Party Licenses

Flutter Demon depends on numerous open-source libraries. This document provides a summary of all third-party dependencies and their licenses.

## License Compatibility

All dependencies use licenses that are compatible with Flutter Demon's Business Source License 1.1 (converting to AGPL-3.0 after 4 years). These are permissive licenses that allow commercial and non-commercial use.

## License Summary

| License Type | Count | Compatibility |
|--------------|-------|---------------|
| Apache-2.0 OR MIT | 212 | ✅ Permissive |
| MIT | 84 | ✅ Permissive |
| MIT OR Unlicense | 5 | ✅ Permissive |
| Apache-2.0 | 5 | ✅ Permissive |
| Apache-2.0 WITH LLVM | 5 | ✅ Permissive |
| ISC | 2 | ✅ Permissive |
| Apache-2.0 OR MIT OR Zlib | 2 | ✅ Permissive |
| CC0-1.0 (Public Domain) | 1 | ✅ Permissive |
| BSD-3-Clause | 1 | ✅ Permissive |
| Zlib | 1 | ✅ Permissive |
| MPL-2.0 | 1 | ✅ Permissive/Copyleft (weak) |
| WTFPL | 1 | ✅ Permissive |

**Total Dependencies**: 320+ packages

## Key Direct Dependencies

### TUI Framework
- **ratatui** (MIT) - Terminal user interface framework
  - Repository: https://github.com/ratatui/ratatui
  - Copyright: © 2016-2025 Ratatui Developers
  
- **crossterm** (MIT) - Cross-platform terminal manipulation
  - Repository: https://github.com/crossterm-rs/crossterm
  - Copyright: © crossterm developers

### Async Runtime
- **tokio** (MIT) - Asynchronous runtime for Rust
  - Repository: https://github.com/tokio-rs/tokio
  - Copyright: © Tokio Contributors

### Serialization
- **serde** (Apache-2.0 OR MIT) - Serialization framework
  - Repository: https://github.com/serde-rs/serde
  - Copyright: © Serde Developers

- **serde_json** (Apache-2.0 OR MIT) - JSON support for Serde
  - Repository: https://github.com/serde-rs/json
  - Copyright: © Serde Developers

- **toml** (Apache-2.0 OR MIT) - TOML parser
  - Repository: https://github.com/toml-rs/toml
  - Copyright: © TOML-rs Developers

### Error Handling
- **color-eyre** (Apache-2.0 OR MIT) - Beautiful error reporting
  - Repository: https://github.com/eyre-rs/eyre
  - Copyright: © Eyre Developers

- **thiserror** (Apache-2.0 OR MIT) - Error derive macros
  - Repository: https://github.com/dtolnay/thiserror
  - Copyright: © David Tolnay

### Logging
- **tracing** (MIT) - Application-level tracing
  - Repository: https://github.com/tokio-rs/tracing
  - Copyright: © Tokio Contributors

- **tracing-subscriber** (MIT) - Tracing utilities
  - Repository: https://github.com/tokio-rs/tracing
  - Copyright: © Tokio Contributors

- **tracing-appender** (MIT) - File appender for tracing
  - Repository: https://github.com/tokio-rs/tracing
  - Copyright: © Tokio Contributors

### Time Handling
- **chrono** (Apache-2.0 OR MIT) - Date and time library
  - Repository: https://github.com/chronotope/chrono
  - Copyright: © Chrono Developers

### CLI Parsing
- **clap** (Apache-2.0 OR MIT) - Command line argument parser
  - Repository: https://github.com/clap-rs/clap
  - Copyright: © Clap Developers

### File Watching
- **notify** (CC0-1.0) - Cross-platform filesystem notification
  - Repository: https://github.com/notify-rs/notify
  - Copyright: Public Domain (CC0-1.0)

- **notify-debouncer-full** (Apache-2.0 OR MIT) - Debouncing for notify
  - Repository: https://github.com/notify-rs/notify
  - Copyright: © Notify Developers

### Utilities
- **dirs** (Apache-2.0 OR MIT) - Platform-specific directories
  - Repository: https://github.com/soc/dirs-rs
  - Copyright: © dirs-rs Developers

- **regex** (Apache-2.0 OR MIT) - Regular expressions
  - Repository: https://github.com/rust-lang/regex
  - Copyright: © Rust Project Developers

- **rand** (Apache-2.0 OR MIT) - Random number generation
  - Repository: https://github.com/rust-random/rand
  - Copyright: © Rand Developers

### Testing (Dev Dependencies)
- **mockall** (Apache-2.0 OR MIT) - Mock object framework
  - Repository: https://github.com/asomers/mockall
  - Copyright: © Alan Somers

- **serial_test** (MIT) - Serial test execution
  - Repository: https://github.com/palfrey/serial_test
  - Copyright: © Tom Parker-Shemilt

- **expectrl** (MIT) - PTY control for testing
  - Repository: https://github.com/zhiburt/expectrl
  - Copyright: © expectrl developers

- **insta** (Apache-2.0) - Snapshot testing
  - Repository: https://github.com/mitsuhiko/insta
  - Copyright: © Armin Ronacher

- **tempfile** (Apache-2.0 OR MIT) - Temporary files and directories
  - Repository: https://github.com/Stebalien/tempfile
  - Copyright: © tempfile Developers

## License Texts

### MIT License
```
Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

### Apache License 2.0
For the full Apache License 2.0 text, see: https://www.apache.org/licenses/LICENSE-2.0

Summary: Permissive license that allows commercial use, modification, distribution, and patent use with conditions requiring preservation of copyright and license notices.

### ISC License
Similar to MIT but with simpler wording. Allows unrestricted use with attribution.

### CC0-1.0 (Creative Commons Zero)
Public domain dedication - no restrictions whatsoever.

### Other Licenses
- **BSD-3-Clause**: Similar to MIT with additional clause about endorsements
- **MPL-2.0**: Weak copyleft - modifications to MPL files must be released, but can be combined with proprietary code
- **Zlib**: Very permissive, similar to MIT
- **WTFPL**: "Do What The F*ck You Want To Public License" - extremely permissive

## Generating Up-to-Date List

To generate a current list of all dependencies and their licenses:

```bash
# Install cargo-license if not already installed
cargo install cargo-license

# List all dependencies with licenses
cargo license --authors

# Export to JSON format
cargo license --json > licenses.json
```

## Attribution Requirements

When distributing Flutter Demon or works derived from it:

1. ✅ Include the `LICENSE` file (contains all required attributions)
2. ✅ Include this `THIRD_PARTY_LICENSES.md` file (optional but recommended)
3. ✅ Preserve copyright notices in source files
4. ✅ Do not remove license headers from dependencies

## Questions?

For questions about licensing:
- Review the main `LICENSE` file in the project root
- See individual dependency repositories for their specific license terms
- Contact the Flutter Demon maintainers for clarification

## Last Updated

This document was last updated on: 2025-01-XX

To verify current dependencies, always run `cargo license` on the latest version.