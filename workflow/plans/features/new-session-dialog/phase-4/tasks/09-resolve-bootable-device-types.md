## Task: Resolve BootableDevice Type Duplication

**Objective**: Unify or clearly separate the two `BootableDevice` types to eliminate confusion and reduce maintenance burden.

**Depends on**: 05-discovery-integration

**Source**: Architecture Enforcer, Risks & Tradeoffs Analyzer (Review Issue #4)

### Scope

- `src/daemon/mod.rs`: Enum `BootableDevice { IosSimulator(...), AndroidAvd(...) }`
- `src/core/types.rs`: Struct `BootableDevice { id, name, platform, runtime, state }`
- `src/app/handler/update.rs`: Conversion logic between types

### Details

Two distinct types with the same name exist in different layers:

**In daemon/mod.rs (line 42):**
```rust
pub enum BootableDevice {
    IosSimulator { udid: String, name: String, runtime: String, state: String },
    AndroidAvd { name: String, api_level: Option<u32> },
}
```

**In core/types.rs (line 667):**
```rust
pub struct BootableDevice {
    pub id: String,
    pub name: String,
    pub platform: Platform,
    pub runtime: Option<String>,
    pub state: DeviceState,
}
```

This creates confusion and requires manual conversion in handlers.

### Options

**Option A (Recommended): Rename daemon type to `BootCommand`**
- Keep `core::BootableDevice` as the canonical UI/state type
- Rename `daemon::BootableDevice` to `daemon::BootCommand` (represents boot capability)
- Add `impl From<BootCommand> for core::BootableDevice`
- Clearest separation of concerns

**Option B: Unify into single core type**
- Remove `daemon::BootableDevice` enum
- Have `list_ios_simulators()` and `list_android_avds()` return `Vec<core::BootableDevice>`
- Simpler but mixes platform-specific details into core

**Option C: Document and add explicit conversion**
- Keep both types, add `From` trait implementations
- Document the design decision in code comments
- Least change but maintains confusion

### Acceptance Criteria

1. No ambiguous `BootableDevice` type references
2. Clear ownership: which layer owns which type
3. Conversion between types is explicit and documented
4. Handler code is simplified (no manual field mapping)
5. `cargo test` passes
6. `cargo clippy -- -D warnings` passes

### Implementation (Option A)

1. Rename `daemon::BootableDevice` to `daemon::BootCommand`:
```rust
// src/daemon/mod.rs
pub enum BootCommand {
    IosSimulator { udid: String, name: String, runtime: String, state: String },
    AndroidAvd { name: String, api_level: Option<u32> },
}
```

2. Add conversion trait:
```rust
impl From<BootCommand> for core::BootableDevice {
    fn from(cmd: BootCommand) -> Self {
        match cmd {
            BootCommand::IosSimulator { udid, name, runtime, state } => {
                BootableDevice {
                    id: udid,
                    name,
                    platform: Platform::Ios,
                    runtime: Some(runtime),
                    state: parse_device_state(&state),
                }
            }
            BootCommand::AndroidAvd { name, api_level } => {
                BootableDevice {
                    id: name.clone(),
                    name,
                    platform: Platform::Android,
                    runtime: api_level.map(|v| format!("API {}", v)),
                    state: DeviceState::Offline,
                }
            }
        }
    }
}
```

3. Update all references from `BootableDevice` to `BootCommand` in daemon code
4. Update handlers to use `.into()` for conversion

### Testing

```rust
#[test]
fn test_boot_command_to_bootable_device_ios() {
    let cmd = BootCommand::IosSimulator {
        udid: "ABC-123".into(),
        name: "iPhone 15".into(),
        runtime: "iOS 17.0".into(),
        state: "Shutdown".into(),
    };
    let device: BootableDevice = cmd.into();
    assert_eq!(device.platform, Platform::Ios);
}
```

### Notes

- This is an architectural decision - confirm approach before implementing
- Search for all usages of both types before making changes
- Consider if `BootCommand` should also have boot methods attached

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `/Users/ed/Dev/zabin/flutter-demon/src/daemon/mod.rs` | Renamed `BootableDevice` enum to `BootCommand`, added doc comments, implemented `From<BootCommand> for core::BootableDevice` trait, updated all tests |
| `/Users/ed/Dev/zabin/flutter-demon/src/core/types.rs` | Added `with_state()` builder method to `BootableDevice` struct |
| `/Users/ed/Dev/zabin/flutter-demon/src/app/handler/update.rs` | Simplified `BootableDevicesDiscovered` handler to use `BootCommand` with `.into()` conversion instead of manual field mapping |

### Notable Decisions/Tradeoffs

1. **Rename to BootCommand**: The daemon type was renamed from `BootableDevice` to `BootCommand` to clearly represent its purpose as a boot capability wrapper, distinct from `core::BootableDevice` which is the UI/state representation.

2. **State Mapping**: Implemented explicit state conversion from `SimulatorState` to `DeviceState` in the `From` trait, ensuring proper state representation across layers.

3. **Builder Pattern**: Added `with_state()` builder method to `core::BootableDevice` for convenient state setting during conversion, following Rust builder pattern conventions.

4. **Simplified Handler**: The handler code was simplified from 26 lines of manual field mapping to 8 lines using `BootCommand` and `.into()`, reducing maintenance burden and improving readability.

### Testing Performed

- `cargo check` - Passed
- `cargo test --lib` - Passed (1452 tests passed; 0 failed; 3 ignored)
- `cargo clippy -- -D warnings` - Passed (no warnings)
- `cargo fmt --check` - Passed (code properly formatted)

### Risks/Limitations

1. **Breaking Change**: This is a breaking change if any external code depends on `daemon::BootableDevice`. However, since this is an internal type not exported in the public API, the risk is minimal.

2. **State Assumptions**: AVDs are assumed to be in `Shutdown` state when discovered via `list_android_avds()`. This is accurate per the function's contract but is worth noting for future maintenance.
