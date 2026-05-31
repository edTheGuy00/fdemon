## Task: Per-route Title / description / canonical / OG

**Objective**: Give each route a unique title, meta description, and canonical so
Googlebot's wave-2 render (and the S07 prerender output) carries accurate per-page
metadata.

**Depends on**: 04-meta-context; website-content-accuracy plan T01/T05/T06 (edit the same
files — land content first)

**Agent:** implementor

**Estimated Time**: 2-3 hours

### Scope

**Files Modified (Write):**
- `website/src/pages/home.rs` and `website/src/pages/docs/*.rs` (all 10): add
  `leptos_meta` tags near the top of each component.

**Files Read (Dependencies):**
- `website/src/lib.rs`: global title formatter from S04.

### Details

Per route, add near the top of the component:
- `<Title text="…">` — short, page-specific (formatter appends " — Flutter Demon").
- `<Meta name="description" content="…">` — 1–2 sentence page summary.
- `<Link rel="canonical" href="https://fdemon.dev/<route>">`.
- (Home only, optional) per-page OG overrides if they should differ from S01 defaults.

Suggested titles/descriptions:

| Route | Title | Description focus |
|-------|-------|-------------------|
| `/` | Flutter Demon — A Rust TUI for Flutter | what it is + key features |
| `/docs` | Documentation | overview/getting started |
| `/docs/installation` | Installation | install via script/cargo, requirements |
| `/docs/keybindings` | Keybindings | full key reference incl. multi-launch |
| `/docs/mouse` | Mouse Support | opt-in mouse interactions |
| `/docs/devtools` | DevTools | Inspector/Performance/Memory/Network |
| `/docs/native-logs` | Native Logs | Android/macOS/iOS capture + orchestrator |
| `/docs/debugging` | Debugging | DAP, stack traces |
| `/docs/configuration` | Configuration | `.fdemon/config.toml` reference |
| `/docs/architecture` | Architecture | 5-crate workspace, TEA |
| `/docs/changelog` | Changelog | release history |

### Acceptance Criteria

1. All 11 routes set a unique `<Title>`, description, and canonical.
2. Titles render with the " — Flutter Demon" suffix (except home if overridden).
3. `cd website && trunk build` compiles; navigating updates head tags.

### Notes

- Must run after the content-accuracy plan's page edits to avoid merge conflicts.
- S09 (landing copy) edits `home.rs` after this task.
</content>
