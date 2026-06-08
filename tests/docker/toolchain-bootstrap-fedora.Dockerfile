# tests/docker/toolchain-bootstrap-fedora.Dockerfile
#
# Fedora variant of the toolchain-bootstrap install-wizard E2E bed — covers the
# **dnf / rpm** package-manager family (Arch is covered by the Manjaro dev box,
# Debian by toolchain-bootstrap.Dockerfile). Same idea: a machine where Flutter /
# JDK / Android / GUI build-deps are ABSENT so the wizard has real work, with the
# basics present so the managed git-clone Flutter install succeeds.
#
# The distro-specific value here is that fdemon's Linux prerequisite detection
# (`detect_linux_package_manager`) should resolve **dnf**, and the Prerequisites
# step should emit dnf-based guided commands (e.g. `sudo dnf install …`).
#
# ── Build all three shell variants (from the repo root) ────────────────────────
#   docker build --build-arg TEST_SHELL=zsh  -t fdemon-bootstrap-fedora-zsh  -f tests/docker/toolchain-bootstrap-fedora.Dockerfile .
#   docker build --build-arg TEST_SHELL=bash -t fdemon-bootstrap-fedora-bash -f tests/docker/toolchain-bootstrap-fedora.Dockerfile .
#   docker build --build-arg TEST_SHELL=fish -t fdemon-bootstrap-fedora-fish -f tests/docker/toolchain-bootstrap-fedora.Dockerfile .
#   # or: tests/docker/build-bootstrap.sh --distro fedora
#
# ── A) Non-interactive detection smoke (confirm dnf is detected) ───────────────
#   docker run --rm fdemon-bootstrap-fedora-zsh fdemon doctor /test-project
#
# ── B) Interactive wizard walk-through ─────────────────────────────────────────
#   docker run -it --rm fdemon-bootstrap-fedora-zsh
#   fdemon            # WORKDIR is /test-project; Prerequisites should show dnf commands
#
# tmux is included; two-pane / docker-exec usage is identical to the Debian bed.
#
# ---------------------------------------------------------------------------
# Stage 1 – Rust builder (Debian/glibc 2.36). A binary built against an older
# glibc runs on Fedora's newer glibc (forward-compatible), so we reuse the same
# builder as the Debian bed rather than bootstrapping rustup on Fedora.
# ---------------------------------------------------------------------------
FROM rust:1.88-bookworm AS builder

WORKDIR /build
# .dockerignore keeps this context small (excludes target/, .git/, docs, etc.).
COPY . .
RUN cargo build --release
# Binary at /build/target/release/fdemon

# ---------------------------------------------------------------------------
# Stage 2 – Bare-ish Fedora runtime (toolchain intentionally incomplete)
# ---------------------------------------------------------------------------
FROM fedora:41

# Basics for a working git-clone Flutter install + all three login shells + tmux.
# NO Flutter, JDK, Android, or GUI build deps (so the wizard's dnf-based
# Prerequisites guidance has work). Installing every shell keeps this dnf layer
# identical across the three TEST_SHELL builds (shared cache).
RUN dnf install -y --setopt=install_weak_deps=False \
    ca-certificates \
    git \
    curl \
    unzip \
    bash \
    zsh \
    fish \
    tmux \
    && dnf clean all

# Select the active login shell. fdemon's HostShell::detect() reads $SHELL and
# matches the basename (bash|zsh|fish), which drives rc_file_for_shell():
#   bash → ~/.bashrc   zsh → ~/.zshenv (pre-created below) else ~/.zprofile
#   fish → ~/.config/fish/config.fish
ARG TEST_SHELL=zsh
ENV SHELL=/usr/bin/${TEST_SHELL}

# Pre-create the canonical rc target so the writer hits the expected file.
RUN case "$TEST_SHELL" in \
      zsh)  touch /root/.zshenv ;; \
      fish) mkdir -p /root/.config/fish ;; \
      bash) : ;; \
      *) echo "unknown TEST_SHELL=$TEST_SHELL (use bash|zsh|fish)" >&2; exit 1 ;; \
    esac

# Minimal runnable Flutter project (pubspec.yaml with a flutter SDK dep + a
# platform dir).
RUN mkdir -p /test-project/linux && \
    printf 'name: test_project\ndescription: Toolchain bootstrap E2E test project\ndependencies:\n  flutter:\n    sdk: flutter\nenvironment:\n  sdk: ">=3.0.0 <4.0.0"\n' \
    > /test-project/pubspec.yaml

COPY --from=builder /build/target/release/fdemon /usr/local/bin/fdemon

WORKDIR /test-project
# Launch the selected login shell (shell-form CMD expands the $SHELL env var).
CMD exec $SHELL
