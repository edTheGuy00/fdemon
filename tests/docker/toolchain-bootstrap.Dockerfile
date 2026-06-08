# tests/docker/toolchain-bootstrap.Dockerfile
#
# End-to-end manual test bed for the **toolchain bootstrap install wizard**,
# parameterized by login shell so the cross-shell PATH/rc-file writers can be
# exercised (bash → ~/.bashrc, zsh → ~/.zshenv, fish → ~/.config/fish/config.fish).
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
#   - bash, zsh, fish       → all three login shells (the ACTIVE one is selected
#                             by the TEST_SHELL build arg via $SHELL)
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
# ── Build all three shell variants (from the repo root, NOT this dir) ──────────
#   docker build --build-arg TEST_SHELL=zsh  -t fdemon-bootstrap-debian-zsh  -f tests/docker/toolchain-bootstrap.Dockerfile .
#   docker build --build-arg TEST_SHELL=bash -t fdemon-bootstrap-debian-bash -f tests/docker/toolchain-bootstrap.Dockerfile .
#   docker build --build-arg TEST_SHELL=fish -t fdemon-bootstrap-debian-fish -f tests/docker/toolchain-bootstrap.Dockerfile .
#   # or just: tests/docker/build-bootstrap.sh
#   # The Rust build layer is compiled once and reused across all three (same context).
#
# ── A) Non-interactive detection smoke test (proves from-scratch preflight) ────
#   docker run --rm fdemon-bootstrap-debian-zsh fdemon doctor /test-project
#   # prints the structured "what's installed / missing" report; exit code:
#   #   0 = healthy for a non-Android project, 1 = a required component degraded.
#
# ── B) Interactive wizard walk-through (the real end-to-end test) ──────────────
#   docker run -it --rm fdemon-bootstrap-debian-zsh        # or -bash / -fish
#   # then inside the container shell:
#   fdemon /test-project
#   #   • wizard opens (no Flutter SDK found)
#   #   • press Enter on "Flutter SDK" → git-clone install streams progress
#   #   • on success it AUTO-configures PATH (the fix): no manual PathConfig step
#   #   • exit the wizard, then verify the rc file for THIS shell:
#   #       zsh  → cat ~/.zshenv
#   #       bash → cat ~/.bashrc
#   #       fish → cat ~/.config/fish/config.fish
#   #     (all should contain /root/fvm/versions/stable/bin)
#
# ── Two panes (run the wizard + commands side by side) ─────────────────────────
#   Option 1 — tmux (single window):
#     docker run -it --rm fdemon-bootstrap-debian-zsh
#     tmux                 # then Ctrl-b " (split) ; run fdemon in one pane,
#                          # commands in the other ; Ctrl-b <arrow> to switch
#   Option 2 — two host terminals via docker exec (shared container state):
#     # terminal 1:
#     docker run -it --name fdemon-test fdemon-bootstrap-debian-zsh   # then: fdemon
#     # terminal 2:
#     docker exec -it fdemon-test "$SHELL"                     # run commands here
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

# Basics for a working git-clone Flutter install + all three login shells + tmux.
# NO Flutter, JDK, Android, or GUI build deps. Installing every shell here keeps
# this apt layer identical across the three TEST_SHELL builds (shared cache).
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    git \
    curl \
    unzip \
    bash \
    zsh \
    fish \
    tmux \
    && rm -rf /var/lib/apt/lists/*

# Select the active login shell. fdemon's HostShell::detect() reads $SHELL and
# matches the basename (bash|zsh|fish), which drives rc_file_for_shell():
#   bash → ~/.bashrc   zsh → ~/.zshenv (pre-created below) else ~/.zprofile
#   fish → ~/.config/fish/config.fish
ARG TEST_SHELL=zsh
ENV SHELL=/usr/bin/${TEST_SHELL}

# Pre-create the canonical rc target so the writer hits the expected file:
#   - zsh: touch ~/.zshenv so it wins over ~/.zprofile (reproduces the bug case)
#   - fish: ensure ~/.config/fish exists (the writer also creates parents)
#   - bash: nothing needed (~/.bashrc is created on write)
RUN case "$TEST_SHELL" in \
      zsh)  touch /root/.zshenv ;; \
      fish) mkdir -p /root/.config/fish ;; \
      bash) : ;; \
      *) echo "unknown TEST_SHELL=$TEST_SHELL (use bash|zsh|fish)" >&2; exit 1 ;; \
    esac

# Minimal runnable Flutter project. `is_runnable_flutter_project` needs a
# pubspec.yaml with a flutter SDK dep AND at least one platform dir (linux/).
RUN mkdir -p /test-project/linux && \
    printf 'name: test_project\ndescription: Toolchain bootstrap E2E test project\ndependencies:\n  flutter:\n    sdk: flutter\nenvironment:\n  sdk: ">=3.0.0 <4.0.0"\n' \
    > /test-project/pubspec.yaml

COPY --from=builder /build/target/release/fdemon /usr/local/bin/fdemon

WORKDIR /test-project
# Launch the selected login shell (shell-form CMD expands the $SHELL env var).
CMD exec $SHELL
