## Task: Add `.github/workflows/ci.yml` with Linux + macOS + Windows runners

**Objective**: Set up GitHub Actions CI on three platforms so we catch Windows-specific regressions before shipping. The project currently has no CI at all — this is why issues #32 / #34 reached production.

**Depends on**: None (independent of code changes)

**Estimated Time**: 1-2h

### Scope

**Files Modified (Write):**
- `.github/workflows/ci.yml` (NEW): GitHub Actions workflow definition.

**Files Read (Dependencies):**
- `docs/DEVELOPMENT.md` (for the canonical "full verification" command sequence — `fmt + check + test + clippy`).
- `Cargo.toml` (workspace MSRV — pin the toolchain to it).

### Details

The workflow runs on three OSes in parallel and gates the merge.

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

env:
  CARGO_TERM_COLOR: always
  RUST_BACKTRACE: 1

jobs:
  check:
    name: Check / ${{ matrix.os }}
    runs-on: ${{ matrix.os }}
    strategy:
      fail-fast: false
      matrix:
        os: [ubuntu-latest, macos-latest, windows-latest]
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust toolchain
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy

      - name: Cache cargo registry, index, and target
        uses: Swatinem/rust-cache@v2

      - name: cargo fmt
        run: cargo fmt --all -- --check

      - name: cargo check
        run: cargo check --workspace --all-targets

      - name: cargo test
        run: cargo test --workspace
        # Some E2E tests rely on PTY; gate behavior matches scripts/test-e2e.sh.

      - name: cargo clippy
        run: cargo clippy --workspace --all-targets -- -D warnings
```

#### Notes on each step

- **`actions/checkout@v4`** — pin to a major; current best-practice as of 2026.
- **`dtolnay/rust-toolchain@stable`** — installs the latest stable. If we want a fixed version, replace `stable` with e.g. `1.77.2` (the MSRV bump from task 02). For a project that ships binaries to users, "latest stable" is appropriate; for libraries, pin tighter. Pick stable.
- **`Swatinem/rust-cache@v2`** — caches `~/.cargo/registry`, `~/.cargo/git`, and `target/`. Speeds incremental CI by ~5×. Industry standard.
- **`cargo test --workspace`** — note that `tests/` integration tests on the binary crate include some that are `#[ignore]`'d by default (per `CLAUDE.md`: "62 ignored — PTY stream timing issues"). `cargo test` does not run ignored tests, so those will not block CI.
- **`cargo clippy --workspace --all-targets`** — covers tests + benches in addition to lib/bin code. Matches the project's quality gate in `docs/DEVELOPMENT.md`.

#### Avoid these pitfalls

- Do **not** install Flutter on the runner. The Windows-specific tests use a fake `flutter.bat` shim (task 05); they don't need a real Flutter SDK. Installing Flutter on each CI run would add 5+ minutes per job and pull in Android/iOS toolchains we don't need.
- Do **not** enable `cargo nextest` in CI yet. It's optional per `docs/DEVELOPMENT.md` and uses a different test-discovery mechanism. Stick with vanilla `cargo test` for now; switch later if test runtime becomes a problem.
- Do **not** include the broken `tests/` E2E tests as a separate job. The 62 ignored ones stay ignored; the rest run as part of `cargo test --workspace`.
- Do **not** push artifacts from this workflow. A separate `release.yml` (out of scope) handles binary artifacts.

### Acceptance Criteria

1. `.github/workflows/ci.yml` exists with the three-OS matrix.
2. Pushing to a feature branch triggers the workflow on all three runners.
3. On a clean `main` build, all jobs pass green.
4. The Windows job runs the new `windows_tests.rs` tests from task 05.
5. The workflow uses `Swatinem/rust-cache@v2` (or equivalent caching).
6. Total wall-clock time per matrix entry is < 10 minutes for a cached run.

### Testing

```bash
# Locally, validate YAML syntax (act not required):
ruby -ryaml -e 'YAML.load_file(".github/workflows/ci.yml")'
# or
python3 -c 'import yaml; yaml.safe_load(open(".github/workflows/ci.yml"))'
```

After merge, verify on the GitHub Actions tab that all three runners pass.

### Notes

- The `pull_request` trigger ensures every PR (including those from forks) is gated by CI.
- `fail-fast: false` keeps non-Windows jobs running even if Windows fails — useful when diagnosing a Windows-specific issue.
- Future enhancement (out of scope): add a `release.yml` that builds and uploads `windows-x86_64`, `macos-x86_64`, `macos-aarch64`, `linux-x86_64` artifacts on tag push. That would let us hand a Windows build to the reporters of #32 / #34 directly from a release page.
- If the team prefers `taiki-e/install-action` over `dtolnay/rust-toolchain`, swap accordingly. Both are widely used; `dtolnay`'s is the simplest.
- This workflow does not run on `windows-2025` or `windows-2019` — `windows-latest` (currently 2022, will roll forward to 2025) is sufficient. Pin if reproducibility becomes critical.
