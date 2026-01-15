## Task: Optimize Navigation Performance

**Objective**: Eliminate unnecessary allocations during device list navigation by caching the flattened list.

**Depends on**: 05-target-selector-messages

**Priority**: Major

**Source**: Code Quality Inspector, Risks Tradeoffs Analyzer - Review Issue #6

### Scope

- `src/tui/widgets/new_session_dialog/target_selector.rs:118-147`: `current_flat_list()` method
- `src/tui/widgets/new_session_dialog/device_groups.rs:152-156, 194`: Device grouping methods

### Problem

`current_flat_list()` creates new Vec allocations with cloned Strings on every navigation operation:

```rust
// target_selector.rs - Called on every Up/Down key press
fn current_flat_list(&self) -> Vec<DeviceListItem> {
    match self.current_tab {
        TargetTab::Connected => {
            self.connected_groups.iter()
                .flat_map(|g| /* clone all items */)
                .collect() // New allocation every time!
        }
        // ...
    }
}
```

**Impact:** Performance degradation with 50+ devices, noticeable on every keystroke.

### Details

Cache the flattened list in state and invalidate on device updates:

```rust
// target_selector.rs
pub struct TargetSelectorState {
    // Existing fields...

    /// Cached flattened device list, invalidated on device updates
    cached_flat_list: Option<Vec<DeviceListItem>>,
}

impl TargetSelectorState {
    /// Returns cached flat list, computing if necessary
    pub fn flat_list(&mut self) -> &[DeviceListItem] {
        if self.cached_flat_list.is_none() {
            self.cached_flat_list = Some(self.compute_flat_list());
        }
        self.cached_flat_list.as_ref().unwrap()
    }

    fn compute_flat_list(&self) -> Vec<DeviceListItem> {
        // Existing current_flat_list() logic
    }

    /// Call when devices change to invalidate cache
    fn invalidate_cache(&mut self) {
        self.cached_flat_list = None;
    }

    pub fn set_connected_devices(&mut self, devices: Vec<Device>) {
        self.connected_devices = devices;
        self.invalidate_cache();
    }

    pub fn set_bootable_devices(&mut self, devices: BootableDevices) {
        self.bootable_devices = devices;
        self.invalidate_cache();
    }

    pub fn set_tab(&mut self, tab: TargetTab) {
        self.current_tab = tab;
        self.invalidate_cache(); // Different tab = different list
    }
}
```

### Acceptance Criteria

1. No new allocations during Up/Down navigation
2. Cache invalidated when devices are updated
3. Cache invalidated when tab is switched
4. Performance improvement verified with benchmark
5. All existing tests pass
6. Memory usage doesn't grow unbounded (cache is single Vec, not accumulating)

### Testing

```rust
#[test]
fn test_navigation_uses_cached_list() {
    let mut state = create_state_with_devices(100);

    // First access computes cache
    let list1 = state.flat_list();
    let ptr1 = list1.as_ptr();

    // Navigation uses cache (same pointer)
    state.navigate_down();
    let list2 = state.flat_list();
    let ptr2 = list2.as_ptr();

    assert_eq!(ptr1, ptr2, "Should use cached list, not reallocate");
}

#[test]
fn test_cache_invalidated_on_device_update() {
    let mut state = create_state_with_devices(10);
    let _ = state.flat_list(); // Populate cache

    state.set_connected_devices(vec![new_device()]);

    assert!(state.cached_flat_list.is_none());
}
```

### Benchmark

Create a simple benchmark:

```rust
#[bench]
fn bench_navigation_100_devices(b: &mut Bencher) {
    let mut state = create_state_with_devices(100);
    b.iter(|| {
        for _ in 0..50 {
            state.navigate_down();
        }
    });
}
```

### Notes

- Consider using `Cow<str>` instead of String cloning for device names
- Alternative: Return iterator instead of Vec (lazy evaluation)
- Cache is per-tab, so switching tabs invalidates
- Watch for subtle bugs where cache isn't invalidated after mutations
