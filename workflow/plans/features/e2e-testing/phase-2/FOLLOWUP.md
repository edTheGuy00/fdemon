# Phase 2 Follow-Up Tasks: Resolving Device Testing Limitations

## Overview

Phase 2 Docker E2E infrastructure is complete, but testing discovered fundamental limitations:

1. **No Flutter devices in Docker**: Flutter requires a connected device (emulator/simulator) to run an app
2. **TUI output not parseable**: fdemon's TUI renders ANSI escape codes, not plaintext events

This document outlines follow-up tasks based on external research into viable solutions.

## Research Summary

### Option A: Flutter Linux Desktop in Docker (Recommended)

Flutter can build and run as a **Linux desktop application** inside Docker using Xvfb (X Virtual Framebuffer).

**Advantages:**
- No need for KVM or nested virtualization
- Fast startup (~2-3 seconds vs 30+ seconds for emulator)
- Lower resource requirements (~200-400MB vs 2-4GB for emulator)
- Works with current Docker infrastructure
- Officially supported by Flutter (used in flutter/plugins CI)

**Requirements:**
- Install `libgtk-3-dev`, `xvfb`, Mesa libraries
- Start Xvfb virtual display before running Flutter
- Build fixtures with `flutter build linux`

### Option B: Android Emulator in Docker

Running Android emulator in Docker is possible but complex.

**Requirements:**
- Linux host with KVM support (`/dev/kvm`)
- Nested virtualization (not available on most cloud CI)
- x86_64 emulator images (ARM is ~10x slower)
- 4GB+ RAM per emulator instance

**Solutions:**
- [google/android-emulator-container-scripts](https://github.com/google/android-emulator-container-scripts)
- [budtmo/docker-android](https://github.com/budtmo/docker-android)

**Limitation:** GitHub Actions standard runners don't support nested virtualization in Docker.

### Option C: Real Emulator on GitHub Actions (Host Machine)

Skip Docker and run emulators directly on GitHub Actions runners.

**Android on Ubuntu:**
- Use `reactivecircus/android-emulator-runner@v2`
- Ubuntu runners support KVM (2-3x faster than macOS)
- AVD snapshot caching reduces startup from 3-5 min to <30 sec

**iOS on macOS:**
- Use `futureware-tech/simulator-action@v4`
- Use `macos-14` (avoid `macos-15` due to iOS 18.0 bugs)
- Built-in Hypervisor.Framework acceleration

### Option D: fdemon Headless Mode (Prerequisite for All Options)

The TUI escape code issue affects ALL testing approaches. fdemon needs a `--headless` or `--machine` flag that outputs structured JSON events instead of TUI rendering.

---

## Follow-Up Task Index

### Priority 1: Enable Basic Docker Testing

| # | Task | Priority | Effort | Description |
|---|------|----------|--------|-------------|
| F1 | [Flutter Linux Desktop Support](tasks/F1-linux-desktop-support.md) | High | Medium | Update Docker to support Flutter Linux desktop with Xvfb |
| F2 | [Update Fixtures for Linux](tasks/F2-update-fixtures-linux.md) | High | Low | Add linux platform support to Flutter fixtures |
| F3 | [Update Test Scripts for Linux](tasks/F3-update-test-scripts.md) | High | Medium | Modify test scripts to use Flutter Linux desktop target |

### Priority 2: fdemon Headless Mode

| # | Task | Priority | Effort | Description |
|---|------|----------|--------|-------------|
| F4 | [Implement fdemon Headless Mode](tasks/F4-fdemon-headless-mode.md) | Critical | High | Add `--headless` flag for JSON event output |
| F5 | [Headless Mode Test Scripts](tasks/F5-headless-test-scripts.md) | High | Medium | Update E2E scripts to use headless mode |

### Priority 3: GitHub Actions Real Emulator Testing (Deferred)

> **Status:** Deferred - Kept for reference. Focus on Wave 1 & 2 first.

| # | Task | Priority | Effort | Description |
|---|------|----------|--------|-------------|
| F6 | [GitHub Actions Android Emulator](tasks/F6-github-android-emulator.md) | Deferred | Medium | Add workflow with `reactivecircus/android-emulator-runner` |
| F7 | GitHub Actions iOS Simulator | Deferred | Medium | Add workflow with `futureware-tech/simulator-action` |
| F8 | AVD Snapshot Caching | Deferred | Low | Implement AVD caching for faster CI |

### Priority 4: Documentation

| # | Task | Priority | Effort | Description |
|---|------|----------|--------|-------------|
| F9 | [Document Testing Strategy](tasks/F9-document-testing-strategy.md) | Medium | Low | Document the E2E testing pyramid and options |

---

## Recommended Implementation Order

### Wave 1: Quick Win - Flutter Linux Desktop
**Goal:** Enable basic Docker E2E testing without device emulation

```
F1 (Linux Desktop Support)
        │
        ├── F2 (Update Fixtures)
        │
        └── F3 (Update Test Scripts)
```

This can be done immediately with current infrastructure.

### Wave 2: Enable Parseable Output
**Goal:** Allow test scripts to verify fdemon behavior

```
F4 (fdemon Headless Mode)
        │
        └── F5 (Headless Test Scripts)
```

This is the **critical path** for meaningful E2E testing.

### Wave 3: Full Mobile Testing (Deferred)
**Goal:** Test with actual Android/iOS devices on CI
**Status:** Deferred - Focus on Flutter Linux Desktop and Headless Mode first.

```
F6 (Android Emulator) ──┬── F8 (AVD Caching)
                        │
F7 (iOS Simulator) ─────┘
```

See `tasks/F6-github-android-emulator.md` for reference implementation.

### Wave 4: Documentation
```
F9 (Document Strategy)
```

---

## Testing Pyramid (Updated)

After implementing follow-up tasks:

```
┌─────────────────────────────────────────────────────────────┐
│  Level 5: Physical Device Testing (Firebase Test Lab)       │
│  - Nightly only, comprehensive device matrix                │
│  - ~30 min, expensive (Future consideration)                │
├─────────────────────────────────────────────────────────────┤
│  Level 4: Real Emulators (GitHub Actions - NEW)            │
│  - Android (Ubuntu KVM) + iOS (macOS)                       │
│  - ~15 min per platform, nightly + manual                   │
├─────────────────────────────────────────────────────────────┤
│  Level 3: Docker with Flutter Linux Desktop (UPDATED)      │
│  - fdemon --headless with Flutter Linux app                 │
│  - ~5-10 min, every PR merge                                │
├─────────────────────────────────────────────────────────────┤
│  Level 2: Mock Daemon Tests (CURRENT - Phase 1)            │
│  - Fast feedback, no Flutter required                       │
│  - <2 min, every push                                       │
├─────────────────────────────────────────────────────────────┤
│  Level 1: Unit + Widget Tests (CURRENT)                    │
│  - TestBackend rendering, component logic                   │
│  - <30 sec, every commit                                    │
└─────────────────────────────────────────────────────────────┘
```

---

## Success Criteria

### Wave 1 Complete When:
- [ ] Dockerfile.test includes Xvfb and Linux desktop dependencies
- [ ] All fixtures have `linux/` platform directory
- [ ] `flutter run -d linux` works in Docker container
- [ ] Test scripts use Linux desktop target

### Wave 2 Complete When:
- [ ] `fdemon --headless` outputs JSON events to stdout
- [ ] Test scripts can grep/parse fdemon output for assertions
- [ ] At least 3 E2E scenarios pass with parseable output

### Wave 3 Complete When: (Deferred)
> Wave 3 is deferred. Criteria kept for future reference.

- [ ] GitHub Actions workflow runs Android emulator tests
- [ ] GitHub Actions workflow runs iOS simulator tests
- [ ] AVD snapshot caching reduces Android startup to <60 sec
- [ ] Both workflows pass on `feat/e2e-testing` branch

### Wave 4 Complete When:
- [ ] E2E testing strategy documented in `docs/TESTING.md`
- [ ] CI workflow options explained for contributors
- [ ] Known limitations documented

---

## Technical Notes

### Flutter Linux Desktop in Docker

```dockerfile
# Add to Dockerfile.test
RUN apt-get update && apt-get install -y \
    clang cmake ninja-build pkg-config \
    libgtk-3-dev libstdc++-12-dev \
    libgl1-mesa-dev libgles2-mesa-dev libegl1-mesa-dev \
    xvfb x11-utils

# Enable Linux desktop
RUN flutter config --enable-linux-desktop
```

```bash
# Start Xvfb and run tests
export DISPLAY=:99
Xvfb :99 -screen 0 1920x1080x24 &
sleep 2
flutter run -d linux
```

### fdemon Headless Mode Output Format

Proposed JSON event format:
```json
{"event": "daemon.connected", "device": "linux", "timestamp": 1704700000}
{"event": "app.started", "session_id": "uuid", "timestamp": 1704700001}
{"event": "hot_reload.started", "timestamp": 1704700010}
{"event": "hot_reload.completed", "duration_ms": 250, "timestamp": 1704700010}
{"event": "log", "level": "info", "message": "Flutter run key commands", "timestamp": 1704700011}
```

### GitHub Actions KVM Setup

```yaml
- name: Enable KVM
  run: |
    echo 'KERNEL=="kvm", GROUP="kvm", MODE="0666", OPTIONS+="static_node=kvm"' | sudo tee /etc/udev/rules.d/99-kvm4all.rules
    sudo udevadm control --reload-rules
    sudo udevadm trigger --name-match=kvm
```

---

## References

- [Flutter Integration Testing Docs](https://docs.flutter.dev/testing/integration-tests)
- [Flutter Linux Desktop Setup](https://docs.flutter.dev/platform-integration/linux/building)
- [reactivecircus/android-emulator-runner](https://github.com/ReactiveCircus/android-emulator-runner)
- [futureware-tech/simulator-action](https://github.com/futureware-tech/simulator-action)
- [google/android-emulator-container-scripts](https://github.com/google/android-emulator-container-scripts)
- [Flutter Plugins PR #2750: Xvfb for Linux tests](https://github.com/flutter/plugins/pull/2750)

---

**Document Version:** 1.0
**Created:** 2026-01-08
**Status:** Draft - Awaiting Approval
