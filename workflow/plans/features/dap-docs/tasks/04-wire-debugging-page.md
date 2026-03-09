## Task: Wire up debugging page in website

**Objective**: Register the new debugging page in the website's routing, sidebar navigation, and add the Bug icon.

**Depends on**: None (can be done in parallel with Task 03; page will compile even before debugging.rs exists if module is declared)

**Estimated Time**: 0.5 hours

### Scope

- `website/src/components/icons.rs`: Add `Bug` icon
- `website/src/pages/docs/mod.rs`: Add module declaration + sidebar entry
- `website/src/lib.rs`: Add route + import

### Details

#### 1. Add Bug icon (`website/src/components/icons.rs`)

Add after the last `lucide_icon!` invocation (currently `ScrollText`):

```rust
lucide_icon!(Bug,
    <path d="m8 2 1.88 1.88" />
    <path d="M14.12 3.88 16 2" />
    <path d="M9 7.13v-1a3.003 3.003 0 1 1 6 0v1" />
    <path d="M12 20c-3.3 0-6-2.7-6-6v-3a4 4 0 0 1 4-4h4a4 4 0 0 1 4 4v3c0 3.3-2.7 6-6 6" />
    <path d="M12 20v-9" />
    <path d="M6.53 9C4.6 8.8 3 7.1 3 5" />
    <path d="M6 13H2" />
    <path d="M3 21c0-2.1 1.7-3.9 3.8-4" />
    <path d="M20.97 5c0 2.1-1.6 3.8-3.5 4" />
    <path d="M22 13h-4" />
    <path d="M17.2 17c2.1.1 3.8 1.9 3.8 4" />
);
```

#### 2. Update docs module (`website/src/pages/docs/mod.rs`)

**a)** Add module declaration at top:
```rust
pub mod debugging;
```

**b)** Add `Bug` to the icon import:
```rust
use crate::components::icons::{Bug, Cpu, Download, Eye, FileText, Keyboard, Menu, ScrollText, Settings};
```

**c)** Add `DocItem` in `doc_items()` — insert after DevTools, before Configuration:
```rust
DocItem {
    href: "/docs/debugging",
    label: "Debugging",
    icon: || view! { <Bug class="w-4 h-4 mr-3" /> }.into_any(),
},
```

Resulting sidebar order:
1. Introduction
2. Installation
3. Keybindings
4. DevTools
5. **Debugging** ← NEW
6. Configuration
7. Architecture
8. Changelog

#### 3. Update router (`website/src/lib.rs`)

**a)** Add import:
```rust
use pages::docs::debugging::Debugging;
```

**b)** Add route inside `<ParentRoute path=path!("/docs") view=DocsLayout>`:
```rust
<Route path=path!("/debugging") view=Debugging />
```

Place after the `/devtools` route for consistency with sidebar order.

### Acceptance Criteria

1. `Bug` icon component exists in `icons.rs`
2. `debugging` module declared in `docs/mod.rs`
3. Sidebar shows "Debugging" with Bug icon between DevTools and Configuration
4. Route `/docs/debugging` renders the `Debugging` component
5. No compilation errors (assuming `debugging.rs` exports `pub fn Debugging`)

### Notes

- The `Bug` SVG paths are from the Lucide icon set (https://lucide.dev/icons/bug) — matches the existing icon convention
- Sidebar order follows a logical flow: UI (Keybindings, DevTools) → Debugging → Configuration → Architecture
