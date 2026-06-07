# tests/docker/toolchain-bootstrap.Dockerfile
#
# End-to-end manual test bed for the **toolchain bootstrap install wizard**.
#
# Unlike the SDK-detection Tier-2 images, this image deliberately ships a
# machine where Flutter, the JDK, the Android SDK, and the Flutter-Linux GUI
# build deps are ALL ABSENT — so the install wizard has real work to do and the
# full guided/auto-install flow can be exercised from scratch.
#
# What IS installed (so the managed Flutter git-clone install can actually
# succeed and produce a usable `flutter`):
#   - ca-certificates, git  → git clone of the Flutter SDK
#   - curl, unzip           → Flutter's first-run Dart SDK bootstrap
#   - zsh + an empty ~/.zshenv + SHELL=/bin/zsh
#                           → reproduces the exact rc-file the bug was about,
#                             so you can watch auto-PATH-config write ~/.zshenv
#   - tmux                  → run the wizard in one pane and commands in another
#                             (Ctrl-b " splits horizontally, Ctrl-b % vertically)
#
# What is deliberately ABSENT (the wizard should flag / install these):
#   - Flutter SDK            → wizard opens; FlutterSdk step installs it
#   - JDK 17                 → guided install command (privileged)
#   - Android cmdline-tools / sdkmanager / platform-tools
#   - clang, cmake, ninja-build, pkg-config, libgtk-3-dev, libglu1-mesa
#                             → Prerequisites step lists them with guided apt cmds
#
# ── Build (from the repo root, NOT this dir) ───────────────────────────────────
#   docker build -t fdemon-bootstrap -f tests/docker/toolchain-bootstrap.Dockerfile .
#
# ── A) Non-interactive detection smoke test (proves from-scratch preflight) ────
#   docker run --rm fdemon-bootstrap fdemon doctor /test-project
#   # prints the structured "what's installed / missing" report; exit code:
#   #   0 = healthy for a non-Android project, 1 = a required component degraded.
#
# ── B) Interactive wizard walk-through (the real end-to-end test) ──────────────
#   docker run -it --rm fdemon-bootstrap
#   # then inside the container shell:
#   fdemon /test-project
#   #   • wizard opens (no Flutter SDK found)
#   #   • press Enter on "Flutter SDK" → git-clone install streams progress
#   #   • on success it AUTO-configures PATH (the fix): no manual PathConfig step
#   #   • exit the wizard, then verify:
#   cat ~/.zshenv          # → contains: export PATH="$PATH:"'/root/fvm/versions/stable/bin'
#   exec zsh && flutter --version   # flutter is now on PATH
#   #   • (optional) run the JDK guided command, then the Android Tools step
#
# ── Two panes (run the wizard + commands side by side) ─────────────────────────
#   Option 1 — tmux (single window):
#     docker run -it --rm fdemon-bootstrap
#     tmux                 # then Ctrl-b " (split) ; run fdemon in one pane,
#                          # commands in the other ; Ctrl-b <arrow> to switch
#   Option 2 — two host terminals via docker exec (shared container state):
#     # terminal 1:
#     docker run -it --name fdemon-test fdemon-bootstrap   # then: fdemon
#     # terminal 2:
#     docker exec -it fdemon-test zsh                      # run commands here
#     # cleanup: docker rm -f fdemon-test
#
# ---------------------------------------------------------------------------
# Stage 1 – Rust builder (produces a Linux x86_64 ELF; matches Debian glibc)
# ---------------------------------------------------------------------------
FROM rust:1.88-bookworm AS builder

WORKDIR /build
# .dockerignore keeps this context small (excludes target/, .git/, docs, etc.).
COPY . .
RUN cargo build --release
# Binary at /build/target/release/fdemon

# ---------------------------------------------------------------------------
# Stage 2 – Bare-ish Debian runtime (toolchain intentionally incomplete)
# ---------------------------------------------------------------------------
FROM debian:bookworm-slim

# Only the basics needed for a git-clone Flutter install to succeed + zsh so we
# reproduce the ~/.zshenv scenario. NO Flutter, JDK, Android, or GUI build deps.
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    git \
    curl \
    unzip \
    zsh \
    tmux \
    && rm -rf /var/lib/apt/lists/*

# Reproduce the user's shell setup: zsh as the login shell with an existing
# (empty) ~/.zshenv so rc_file_for_shell() targets ~/.zshenv (not ~/.zprofile).
ENV SHELL=/bin/zsh
RUN touch /root/.zshenv

# Minimal runnable Flutter project. `is_runnable_flutter_project` needs a
# pubspec.yaml with a flutter SDK dep AND at least one platform dir (linux/).
RUN mkdir -p /test-project/linux && \
    printf 'name: test_project\ndescription: Toolchain bootstrap E2E test project\ndependencies:\n  flutter:\n    sdk: flutter\nenvironment:\n  sdk: ">=3.0.0 <4.0.0"\n' \
    > /test-project/pubspec.yaml

COPY --from=builder /build/target/release/fdemon /usr/local/bin/fdemon

WORKDIR /test-project
CMD ["/bin/zsh"]
