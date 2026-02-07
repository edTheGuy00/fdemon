# Plan: Cyber-Glass Design System Overhaul

## TL;DR

Overhaul the fdemon TUI design system from ad-hoc hardcoded colors to a centralized "Cyber-Glass" theme with RGB color depth hierarchy, rounded glass containers, simulated elevation/shadows, Nerd Font glyphs, and metadata bars. Targets the main log screen, New Session modal, and Settings panel, replicating the design references at `tmp/redesign/logs-and-launch-modal-focus.tsx` and `tmp/redesign/settings-page-focus.tsx`.

---

## Background

The current TUI has **no centralized theme system**. Colors are hardcoded across 15+ widget files using basic named colors (`Color::Cyan`, `Color::DarkGray`, etc.). There are 5 separate style definition locations, each scoped to a single widget. The same semantic color (e.g., "focused border = Cyan") is repeated in 8+ files without any shared constant. This makes the UI look dated and makes visual iteration expensive.

**Current pain points:**
- No visual depth — everything feels flat with uniform `Color::DarkGray` borders
- No centralized palette — changing the accent color requires editing dozens of files
- No focus/elevation hierarchy — active vs inactive panels look nearly identical
- Basic border types only (`BorderType::Plain`, `symbols::border::ROUNDED`)
- No Nerd Font icons — just text labels

**Design reference:** The React/TSX mockup at `tmp/redesign/logs-and-launch-modal-focus.tsx` demonstrates a modern "Cyber-Glass" aesthetic with layered depth, RGB colors, rounded glass containers, glow effects, and rich iconography.

---

## Ratatui Feasibility Assessment

Research confirms ratatui v0.30 (currently in `Cargo.toml`) supports all required features:

| Feature | Ratatui Support | Implementation |
|---------|----------------|----------------|
| **RGB True Color** | `Color::Rgb(r, g, b)` | Direct — requires true-color terminal (standard on modern terminals) |
| **Rounded Borders** | `BorderType::Rounded` → `╭╮╰╯` | Direct — already used in some modals |
| **Thick/Double Borders** | `BorderType::Thick` → `┏┓┗┛`, `BorderType::Double` → `╔╗╚╝` | Direct — for focus states |
| **Block Titles** | `Block::title()`, `Block::title_bottom()` | Direct — for metadata bars |
| **Styled Block Borders** | `Block::border_style(Style)` | Direct — different colors per focus state |
| **Background Fills** | `Block::style(Style::default().bg(color))` | Direct — for depth layers |
| **Clear Widget** | `ratatui::widgets::Clear` | Direct — already used for modal overlays |
| **Rich Text** | `Span`, `Line`, `Text` with mixed styles | Direct — for inline badges, colored tags |
| **Unicode/Nerd Fonts** | Full Unicode support in `Span` text | Direct — embed Nerd Font glyphs as string literals |
| **Shadow Effect** | Manual buffer cell manipulation | Custom — render darker rect offset by (1,1) before modal |
| **Dim Background** | Iterate buffer cells, reduce brightness | Custom — loop over cells and apply dim style |
| **Gradient Simulation** | Multiple styled spans in a line | Approximate — use 2-3 color stops for buttons |

**Limitations:**
- No real blur/transparency — simulated with background color hierarchy
- No subpixel rendering — shadows are 1-cell resolution
- No animation framerate control — blinking cursor via `Modifier::SLOW_BLINK` or tick-based toggling
- Gradient buttons are approximate (2-3 color bands, not smooth)

---

## Affected Modules

### New Files

- `crates/fdemon-tui/src/theme/mod.rs` — **NEW** Centralized theme module (palette, semantic colors, component styles)
- `crates/fdemon-tui/src/theme/palette.rs` — **NEW** Raw color constants (RGB values)
- `crates/fdemon-tui/src/theme/styles.rs` — **NEW** Semantic style builders (borders, text, status indicators)
- `crates/fdemon-tui/src/theme/icons.rs` — **NEW** Nerd Font glyph constants
- `crates/fdemon-tui/src/widgets/modal_overlay.rs` — **NEW** Shared overlay widget (dim + shadow + centered content)

### Modified Files

| File | Changes |
|------|---------|
| `crates/fdemon-tui/src/lib.rs` | Add `pub mod theme;` |
| `crates/fdemon-tui/src/layout.rs` | Update layout to support header metadata bar + bottom status patterns |
| `crates/fdemon-tui/src/render/mod.rs` | Use theme colors for background, update overlay rendering |
| `crates/fdemon-tui/src/widgets/header.rs` | Complete redesign: pulsing dot, project name, shortcut hints, device pill |
| `crates/fdemon-tui/src/widgets/tabs.rs` | Redesign session tabs with glass container style |
| `crates/fdemon-tui/src/widgets/log_view/mod.rs` | Redesign: glass container, header bar, live feed badge, styled entries |
| `crates/fdemon-tui/src/widgets/log_view/styles.rs` | Migrate to theme system |
| `crates/fdemon-tui/src/widgets/status_bar/mod.rs` | Redesign: running dot, mode badge, uptime, error count |
| `crates/fdemon-tui/src/widgets/new_session_dialog/mod.rs` | Complete redesign: glass modal, shadow, dim overlay, new layout |
| `crates/fdemon-tui/src/widgets/new_session_dialog/target_selector.rs` | Redesign: tab toggle, categorized device list with icons |
| `crates/fdemon-tui/src/widgets/new_session_dialog/launch_context.rs` | Redesign: glass fields, mode buttons with glow, gradient launch button |
| `crates/fdemon-tui/src/widgets/new_session_dialog/tab_bar.rs` | Redesign: pill-style toggle (Connected/Bootable) |
| `crates/fdemon-tui/src/widgets/new_session_dialog/device_list.rs` | Redesign: category headers, icon+name rows, selected highlight |
| `crates/fdemon-tui/src/widgets/new_session_dialog/fuzzy_modal.rs` | Migrate styles to theme |
| `crates/fdemon-tui/src/widgets/new_session_dialog/dart_defines_modal.rs` | Migrate styles to theme |
| `crates/fdemon-tui/src/widgets/confirm_dialog.rs` | Redesign with glass container + shadow |
| `crates/fdemon-tui/src/widgets/search_input.rs` | Migrate styles to theme |
| `crates/fdemon-tui/src/widgets/settings_panel/mod.rs` | Migrate styles to theme |
| `crates/fdemon-tui/src/widgets/settings_panel/styles.rs` | Migrate to theme system |
| `crates/fdemon-tui/src/selector.rs` | Migrate styles to theme |

---

## Design Token Reference

### Color Palette

```
DEEPEST_BG       = Rgb(10, 12, 16)    // #0a0c10 — terminal background
CARD_BG          = Rgb(18, 21, 28)    // #12151c — panel/card backgrounds
POPUP_BG         = Rgb(28, 33, 43)    // #1c212b — modal/popup backgrounds
SURFACE          = Rgb(22, 27, 34)    // #161b22 — elevated surface

BORDER_DIM       = Rgb(45, 51, 59)    // #2d333b — inactive borders
BORDER_ACTIVE    = Rgb(88, 166, 255)  // #58a6ff — focused borders (accent)

ACCENT           = Rgb(88, 166, 255)  // #58a6ff — primary accent blue
ACCENT_DIM       = Rgb(56, 107, 163)  // #386ba3 — dimmed accent

TEXT_PRIMARY     = Rgb(201, 209, 217) // #c9d1d9 — primary text (slate-300)
TEXT_SECONDARY   = Rgb(125, 133, 144) // #7d8590 — secondary text (slate-500)
TEXT_MUTED       = Rgb(72, 79, 88)    // #484f58 — muted text (slate-600)
TEXT_BRIGHT      = Rgb(240, 246, 252) // #f0f6fc — bright/white text

STATUS_GREEN     = Rgb(16, 185, 129)  // #10b981 — running/success (emerald)
STATUS_RED       = Rgb(244, 63, 94)   // #f43f5e — error/stopped (rose)
STATUS_YELLOW    = Rgb(234, 179, 8)   // #eab308 — warning/reloading
STATUS_BLUE      = Rgb(56, 189, 248)  // #38bdf8 — info (sky)
STATUS_INDIGO    = Rgb(129, 140, 248) // #818cf8 — flutter messages (indigo)

SHADOW           = Rgb(5, 6, 8)       // #050608 — shadow color (near-black)
DIM_OVERLAY      = Rgb(0, 0, 0)       // rendered at 40% by cell manipulation

GRADIENT_BLUE    = Rgb(37, 99, 235)   // #2563eb — button gradient start
GRADIENT_INDIGO  = Rgb(99, 102, 241)  // #6366f1 — button gradient end
```

### Nerd Font Icons

```
ICON_TERMINAL    = "\u{f120}"  //  — terminal
ICON_SMARTPHONE  = "\u{f3cd}"  //  — mobile device
ICON_GLOBE       = "\u{f0ac}"  //  — web browser
ICON_MONITOR     = "\u{f108}"  //  — desktop
ICON_ACTIVITY    = "\u{f0f1}"  //  — activity/pulse
ICON_PLAY        = "\u{f04b}"  //  — play/launch
ICON_STOP        = "\u{f04d}"  //  — stop
ICON_REFRESH     = "\u{f021}"  //  — reload
ICON_ALERT       = "\u{f071}"  //  — warning triangle
ICON_CHECK       = "\u{f00c}"  //  — check mark
ICON_CLOSE       = "\u{f00d}"  //  — close/x
ICON_CHEVRON_R   = "\u{f054}"  //  — chevron right
ICON_CHEVRON_D   = "\u{f078}"  //  — chevron down
ICON_DOT         = "\u{f444}"  //  — filled circle (status dot)
ICON_LAYERS      = "\u{f5fd}"  //  — layers/sessions
ICON_CPU         = "\u{f2db}"  //  — cpu/device
ICON_SETTINGS    = "\u{f013}"  //  — settings gear
ICON_ZAP         = "\u{f0e7}"  //  — lightning bolt (behavior)
ICON_EYE         = "\u{f06e}"  //  — eye (watcher)
ICON_CODE        = "\u{f121}"  //  — code brackets (editor)
ICON_USER        = "\u{f007}"  //  — user (session)
ICON_INFO        = "\u{f05a}"  //  — info circle
ICON_KEYBOARD    = "\u{f11c}"  //  — keyboard
ICON_COMMAND     = "\u{f120}"  //  — command/terminal
ICON_SAVE        = "\u{f0c7}"  //  — floppy disk (save)
```

### Border Conventions

| Context | Border Type | Border Color |
|---------|------------|--------------|
| Inactive panel | `BorderType::Rounded` | `BORDER_DIM` |
| Focused/active panel | `BorderType::Rounded` | `ACCENT` |
| Modal outer frame | `BorderType::Rounded` | `Rgb(255, 255, 255)` at 10% (approximate: `BORDER_DIM`) |
| Nested focus (e.g., selected mode button) | No border or `BorderType::Plain` | `ACCENT` |
| Heavy emphasis (rare) | `BorderType::Double` | `ACCENT` |

---

## Development Phases

### Phase 1: Theme Foundation

**Goal**: Create the centralized theme module and migrate all color/style definitions to use it. No visual changes yet — just infrastructure.

#### Steps

1. **Create theme module structure**
   - `crates/fdemon-tui/src/theme/mod.rs` — public API, re-exports
   - `crates/fdemon-tui/src/theme/palette.rs` — all `Color::Rgb()` constants from design tokens above
   - `crates/fdemon-tui/src/theme/styles.rs` — semantic style builders:
     - `fn border_inactive() -> Style`
     - `fn border_active() -> Style`
     - `fn text_primary() -> Style`
     - `fn text_muted() -> Style`
     - `fn status_style(phase: &AppPhase) -> (Style, &str)` — consolidate duplicated phase→color mapping
     - `fn glass_block(focused: bool) -> Block` — standard glass container factory
     - `fn modal_block(title: &str) -> Block` — modal container factory
   - `crates/fdemon-tui/src/theme/icons.rs` — Nerd Font glyph constants

2. **Create shared overlay utilities**
   - `crates/fdemon-tui/src/widgets/modal_overlay.rs` — reusable overlay widget:
     - `dim_background(buf, area)` — iterate cells and apply dim style
     - `render_shadow(buf, modal_rect)` — render 1-cell dark offset
     - `centered_rect(percent_x, percent_y, area) -> Rect` — consolidate existing helpers

3. **Migrate existing style definitions**
   - Replace `log_view/styles.rs` constants with theme references
   - Replace `settings_panel/styles.rs` functions with theme-delegated functions
   - Replace `launch_context.rs::LaunchContextStyles` with theme references
   - Replace `device_list.rs::DeviceListStyles` with theme references
   - Replace `fuzzy_modal.rs::mod styles` with theme references
   - Replace `dart_defines_modal.rs::mod styles` with theme references
   - Replace all inline `Color::Cyan`, `Color::DarkGray`, etc. across 15 widget files

4. **Consolidate phase→icon/color mapping**
   - Create `theme::styles::phase_indicator(phase: &AppPhase) -> (String, Style)`
   - Remove 5 duplicated mappings in `tabs.rs` (3 locations) and `status_bar/mod.rs` (2 locations)

**Milestone**: All widgets render using theme constants. Visual appearance is similar to current (mapped colors). Zero regressions. Single place to change any color.

---

### Phase 2: Main Log Screen Redesign

**Goal**: Transform the main log screen (header + log panel + status bar) to match the Cyber-Glass design.

#### Steps

1. **Update terminal background**
   - In `render/mod.rs::view()`: fill the entire frame area with `DEEPEST_BG` background before rendering widgets
   - This establishes the depth foundation

2. **Redesign MainHeader widget** (`widgets/header.rs`)
   - Layout: single row, `CARD_BG` background, rounded border with `BORDER_DIM`
   - Left side: pulsing green status dot (`ICON_DOT` + `STATUS_GREEN`) + "Flutter Demon" bold + "/" separator + project name in `TEXT_SECONDARY`
   - Center: keyboard shortcut hints (`[r] Run`, `[R] Restart`, `[x] Stop`, `[d] Debug`, `[q] Quit`) in `TEXT_MUTED`, highlight on hover concept not applicable (show all dimmed)
   - Right side: device pill — rounded container with icon (`ICON_SMARTPHONE`/`ICON_GLOBE`/`ICON_MONITOR`) + device name in `ACCENT`
   - Session tabs: integrated below the header line when multiple sessions exist

3. **Redesign LogView widget** (`widgets/log_view/mod.rs`)
   - Glass container: `CARD_BG` background, `BorderType::Rounded`, `BORDER_DIM` border
   - Top metadata bar (1 line): `ICON_TERMINAL` + "TERMINAL LOGS" in `TEXT_SECONDARY` uppercase tracking + right-aligned "LIVE FEED" badge (`CARD_BG` darker bg + `TEXT_MUTED` text)
   - Log entries: timestamp in `TEXT_MUTED` + "bullet" separator + colored tag `[app]`/`[flutter]`/`[watch]` + message in `TEXT_PRIMARY`
   - Tag colors: app=`STATUS_GREEN`, flutter=`STATUS_INDIGO`, watch=`STATUS_BLUE`, error=`STATUS_RED`
   - Blinking cursor line: small block character with `ACCENT` + `Modifier::SLOW_BLINK` at current position
   - Bottom metadata bar (1 line): left=running dot + "Running" in `STATUS_GREEN` + mode badge "Debug (develop)" in `ACCENT`; right=uptime with `ICON_ACTIVITY` + error count with `ICON_ALERT` in `STATUS_RED`

4. **Redesign StatusBar** (`widgets/status_bar/mod.rs`)
   - This becomes the bottom metadata bar of the log panel (merged into LogView's footer), or kept as a separate thin bar below
   - Decision: **merge into LogView footer** — the design shows status info inside the log panel border, not as a separate bar
   - Update `layout.rs` to remove dedicated status bar row if merged, or keep 1-line external bar for compact info

5. **Update Layout** (`layout.rs`)
   - Header: 3 lines (1 for border-top + content + border-bottom of glass container)
   - Logs: flexible, now includes top+bottom metadata bars inside its own border
   - Status: 0 lines (merged) or 1 line (minimal external)
   - Add 1-cell padding/gap between header and log panel for visual breathing room

**Milestone**: Main log screen matches the Cyber-Glass design. Header shows device pill and shortcuts. Log panel has glass container with metadata bars. Status info is integrated.

---

### Phase 3: New Session Modal Redesign

**Goal**: Transform the New Session modal to match the Cyber-Glass design with glass overlay, shadow, and refined layout.

#### Steps

1. **Implement modal overlay system**
   - When `UiMode::NewSessionDialog` or `UiMode::Startup`:
     - First render the background UI normally
     - Then call `dim_background()` to darken all cells (simulate `bg-black/40 backdrop-blur`)
     - Calculate centered modal rect (80% width, 70% height for horizontal; clamp to max 100 cols)
     - Call `render_shadow()` to draw 1-cell dark offset to right+bottom of modal rect
     - Render modal content

2. **Redesign modal frame** (`widgets/new_session_dialog/mod.rs`)
   - Outer frame: `BorderType::Rounded`, border color `BORDER_DIM`, bg `POPUP_BG`
   - Modal header (inside top): 2-line area with:
     - "New Session" in `TEXT_BRIGHT` bold, large-ish
     - "Configure deployment target and runtime flags." in `TEXT_SECONDARY`
     - Right-aligned close button: `ICON_CLOSE` in `TEXT_SECONDARY`
   - Header separator: horizontal line in `BORDER_DIM`

3. **Redesign TargetSelector (left panel)** (`widgets/new_session_dialog/target_selector.rs`)
   - Width: 40% of modal inner area
   - Right border separator: vertical line in `BORDER_DIM`
   - **Tab toggle** at top: pill-style toggle bar
     - Background: darker area (`Rgb(0,0,0)` at ~40% approx `Rgb(12,14,18)`)
     - Active tab: `ACCENT` bg + `TEXT_BRIGHT` text, slight "glow" via lighter border
     - Inactive tab: transparent bg + `TEXT_SECONDARY` text
     - Labels: "1 Connected" / "2 Bootable"
   - **Device list** below tabs:
     - Category headers: small uppercase text in `ACCENT` dimmed (`ACCENT_DIM`) with wider tracking
     - Device rows: `ICON_SMARTPHONE`/`ICON_GLOBE`/`ICON_MONITOR` + device name
     - Selected device: `ACCENT` bg at 10% + `ACCENT` border + brighter text
     - Unselected: transparent bg + `TEXT_SECONDARY` text
     - Category grouping: "iOS Simulators", "Web", "Desktop", "Android"

4. **Redesign LaunchContext (right panel)** (`widgets/new_session_dialog/launch_context.rs`)
   - Background: slightly darker than modal bg (simulate `bg-black/20`)
   - **Configuration dropdown**: label "CONFIGURATION" in `TEXT_SECONDARY` uppercase + field with `SURFACE` bg, `BORDER_DIM` border, `ICON_CHEVRON_D`
   - **Flavor dropdown**: same pattern
   - **Mode selector**: label "MODE" + 3 buttons:
     - Selected mode: `ACCENT` bg at 20% + `ACCENT` border + `ACCENT` text + subtle glow shadow
     - Unselected: transparent bg + `BORDER_DIM` border + `TEXT_SECONDARY` text
   - **Entry Point field**: label + `SURFACE` bg field with "main.dart" + `ICON_CHEVRON_R`
   - **Launch button**: full-width, prominent:
     - Background: `GRADIENT_BLUE` (single color since true gradient isn't feasible)
     - Text: `TEXT_BRIGHT` bold uppercase "LAUNCH INSTANCE" with `ICON_PLAY`
     - Border: none or `GRADIENT_BLUE` with slight lighter variant

5. **Redesign modal footer**
   - Thin bar at bottom inside modal border
   - `SURFACE` background
   - Centered key hints: `[1/2]` Tab, `[Tab]` Pane, `[up/down]` Navigate, `[Enter]` Select, `[Esc]` Close
   - Keys rendered in small "kbd" style: `POPUP_BG` bg + `BORDER_DIM` border around key text
   - Labels in `TEXT_MUTED`

**Milestone**: New Session modal matches the Cyber-Glass design. Glass overlay with dim background and shadow. Two-pane layout with tab toggle, categorized devices, configuration fields, and prominent launch button.

---

### Phase 4: Settings Panel Redesign

**Goal**: Transform the full-screen Settings panel to match the Cyber-Glass design, replicating `tmp/redesign/settings-page-focus.tsx`.

**Design Reference Analysis** (`tmp/redesign/settings-page-focus.tsx`):
- Full-screen replacement view (not a modal overlay — replaces the log view area)
- Same glass container as the log panel (`CARD_BG` bg, `BorderType::Rounded`, `BORDER_DIM` border)
- Header area with settings icon + "System Settings" title + tab bar + `[Esc] Close` hint
- Tab bar: 4 tabs (`1. Project`, `2. User`, `3. Launch`, `4. VSCode`) styled as rounded-top pill buttons
  - Active tab: `ACCENT` bg (`bg-blue-600`) + `TEXT_BRIGHT` text + top/side borders
  - Inactive tabs: transparent bg + `TEXT_SECONDARY` text
- Content area: scrollable, max-width centered, grouped settings
- Setting groups: icon + uppercase category header in `ACCENT_DIM`
- Setting rows: 3-column grid layout:
  - Column 1 (200px): label in `TEXT_PRIMARY` (selected) or `TEXT_SECONDARY` (unselected)
  - Column 2 (150px): value in monospace, color-coded (bool=`STATUS_GREEN`, number=`ACCENT`, etc.)
  - Column 3 (flex): description in `TEXT_MUTED` italic
  - Selected row: `ACCENT` bg at 10% + left border accent bar (2px `ACCENT`)
  - Unselected row: transparent bg + transparent left border
- User tab: info banner (blue tinted glass: `ACCENT` bg at 10% + `ACCENT` border at 20%)
- Launch tab: empty state with centered icon + message
- Footer bar: `DEEPEST_BG`/dark bg, centered shortcut hints
  - `Tab:` Switch tabs, `j/k:` Navigate, `Enter:` Edit, `Ctrl+S:` Save Changes

#### Steps

1. **Redesign Settings header** (`widgets/settings_panel/mod.rs::render_header`)
   - Glass container header area with `SURFACE` bg
   - Left: `ICON_SETTINGS` in `ACCENT` + "System Settings" in `TEXT_BRIGHT` bold
   - Tab bar: horizontal row of tab buttons
     - Active: `ACCENT` bg + `TEXT_BRIGHT` + `BorderType::Rounded` top corners (simulated with `╭─╮` chars or just bg color)
     - Inactive: no bg + `TEXT_SECONDARY`
   - Right: `[Esc]` kbd badge + "Close" in `TEXT_MUTED`

2. **Redesign Settings content area** (`widgets/settings_panel/mod.rs::render_content`)
   - Background: `CARD_BG` (consistent with log panel)
   - Content max-width centered (clamp to ~100 cols on wide terminals)
   - **Setting group headers**: icon glyph + uppercase text in `ACCENT_DIM` with letter spacing
     - Group icons from design: Behavior=`ICON_ZAP` (⚡), Watcher=`ICON_EYE` (👁), UI=`ICON_MONITOR`, Editor=`ICON_CODE`, Session=`ICON_USER`
   - **Setting rows**: 3-column layout using `Layout::horizontal`:
     - `Constraint::Length(25)` — label
     - `Constraint::Length(15)` — value (monospace, color-coded)
     - `Constraint::Fill(1)` — description (italic, `TEXT_MUTED`)
   - **Selected row indicator**: left border bar in `ACCENT` (render `▎` or `│` in accent color at column 0) + subtle `ACCENT` bg tint
   - **Value coloring**:
     - `true` → `STATUS_GREEN`
     - `false` → `STATUS_RED`
     - Numbers → `ACCENT`
     - Strings → `TEXT_PRIMARY`
     - `<empty>` → `TEXT_MUTED`
     - Enums → `STATUS_INDIGO`
     - Lists → `STATUS_BLUE`
     - Override marker `*` → `STATUS_YELLOW`

3. **Redesign User tab info banner**
   - Glass info box: `ACCENT` bg at 10% (approx `Rgb(17, 25, 40)`) + `ACCENT_DIM` border
   - Left: `ICON_INFO` (ℹ) in `ACCENT`
   - Content: "Local Settings Active" in `TEXT_BRIGHT` bold + path in `ACCENT_DIM` monospace

4. **Redesign Launch tab empty state**
   - Centered vertically in content area
   - Large icon: `ICON_LAYERS` in `TEXT_MUTED` inside a subtle circular border (simulated with box border)
   - Title: "No launch configurations found" in `TEXT_PRIMARY`
   - Subtitle: "Create .fdemon/launch.toml or press 'n' to create one." in `TEXT_MUTED` italic

5. **Redesign Settings footer** (`widgets/settings_panel/mod.rs::render_footer`)
   - Background: darker than content (`DEEPEST_BG` or `Rgb(0,0,0)` at 40%)
   - Top border: `BORDER_DIM` horizontal line
   - Centered shortcut hints with icons:
     - `ICON_KEYBOARD` + "Tab:" in `TEXT_SECONDARY` + "Switch tabs" in `TEXT_MUTED`
     - `ICON_COMMAND` + "j/k:" in `TEXT_SECONDARY` + "Navigate" in `TEXT_MUTED`
     - `ICON_CHEVRON_R` + "Enter:" in `TEXT_SECONDARY` + "Edit" in `TEXT_MUTED`
     - `ICON_SAVE` + "Ctrl+S:" in `ACCENT` + "Save Changes" in `TEXT_MUTED`

6. **Migrate settings_panel/styles.rs to theme**
   - Replace all hardcoded colors (`Color::Green`, `Color::Cyan`, `Color::DarkGray`, etc.) with theme palette references
   - Update `value_style()`, `label_style()`, `section_header_style()`, etc. to use theme colors
   - Remove redundant style functions that duplicate theme functionality

7. **Add new icon constants to theme**
   - `ICON_ZAP`, `ICON_EYE`, `ICON_CODE`, `ICON_USER`, `ICON_INFO`, `ICON_KEYBOARD`, `ICON_COMMAND`, `ICON_SAVE`
   - With ASCII fallbacks

**Milestone**: Settings panel matches the Cyber-Glass design. Tabbed header with pill-style tabs, grouped settings with 3-column layout, info banners, empty states, and themed footer. Consistent with the rest of the redesigned UI.

---

### Phase 5: Polish and Remaining Widgets

**Goal**: Apply the Cyber-Glass theme to remaining widgets and polish edge cases.

#### Steps

1. **Redesign ConfirmDialog** — glass container with shadow, themed buttons
2. **Redesign SearchInput** — glass bar style at bottom of log panel
3. **Redesign Loading Screen** — glass container with themed spinner
4. **Redesign Project Selector** — glass list with themed items
5. **Update FuzzyModal** — apply theme colors and glass container style
6. **Update DartDefinesModal** — apply theme colors and glass container style
7. **Test responsive behavior** — verify compact/wide modes still work with new theme
8. **Update snapshot tests** — all existing render tests will need updated expected output

**Milestone**: Entire TUI uses the Cyber-Glass design system consistently. All tests pass.

---

## Edge Cases & Risks

### Terminal True-Color Support
- **Risk:** Some terminals don't support 24-bit RGB colors. `Color::Rgb()` may fall back to nearest 256-color match.
- **Mitigation:** The design degrades gracefully — ratatui/crossterm handle fallback automatically. Colors will be approximate but functional. Document minimum terminal requirements.

### Performance
- **Risk:** `dim_background()` iterating all buffer cells on every frame could be slow on large terminals.
- **Mitigation:** Only iterate when modal is visible. The operation is O(width * height) which is trivially fast (< 1ms for 200x50 terminal). Profile if needed.

### Nerd Font Availability
- **Risk:** Users without Nerd Fonts will see missing glyphs (squares/tofu).
- **Mitigation:** Provide ASCII fallback constants in `theme/icons.rs`. Add a `settings.ui.nerd_fonts = true/false` config option to toggle between Nerd Font and ASCII icons. Default to ASCII for safety; document how to enable Nerd Fonts.

### Test Breakage
- **Risk:** All 427 TUI widget tests use hardcoded buffer assertions that will break when colors change.
- **Mitigation:** Phase 1 task explicitly updates all tests. Use `insta` snapshot tests where possible to make future theme changes easier.

### Compact/Small Terminal
- **Risk:** The new design with metadata bars may not fit in very small terminals.
- **Mitigation:** Keep existing `LayoutMode::Compact` logic. For terminals < 60 cols, fall back to simplified layout without metadata bars. Test at 40x15 minimum.

---

## Configuration Additions

```toml
# .fdemon/config.toml
[ui]
# Enable Nerd Font icons (requires Nerd Font installed)
nerd_fonts = false
```

---

## Success Criteria

### Phase 1 Complete When:
- [ ] `crates/fdemon-tui/src/theme/` module exists with `palette.rs`, `styles.rs`, `icons.rs`
- [ ] All 15+ widget files import colors from `theme::` instead of hardcoded values
- [ ] Phase→icon/color mapping consolidated to single location
- [ ] `cargo test --workspace` passes with zero regressions
- [ ] `cargo clippy --workspace` passes with no warnings

### Phase 2 Complete When:
- [ ] Terminal background uses `DEEPEST_BG` (#0a0c10)
- [ ] Header shows pulsing dot, project name, shortcut hints, device pill
- [ ] Log panel is a glass container with top/bottom metadata bars
- [ ] Log entries show colored timestamps, tags, and messages matching design
- [ ] Status info is integrated into log panel footer
- [ ] All existing functionality preserved (scroll, search, filter, links)

### Phase 3 Complete When:
- [ ] New Session modal renders with dim background overlay
- [ ] Modal has shadow effect (1-cell dark offset)
- [ ] Left panel: tab toggle + categorized device list with icons
- [ ] Right panel: configuration/flavor dropdowns, mode selector, entry point, launch button
- [ ] Footer shows keyboard shortcut hints in "kbd" style
- [ ] All existing functionality preserved (tab switching, device selection, fuzzy search)

### Phase 4 Complete When:
- [ ] Settings panel header shows icon + "System Settings" + pill-style tab bar
- [ ] Active tab uses `ACCENT` bg, inactive tabs use `TEXT_SECONDARY`
- [ ] Setting groups have icon + uppercase category header in `ACCENT_DIM`
- [ ] Setting rows use 3-column layout (label, value, description)
- [ ] Selected row has left accent bar + tinted background
- [ ] User tab info banner renders with glass style
- [ ] Launch tab empty state renders centered icon + message
- [ ] Footer shows themed shortcut hints
- [ ] `settings_panel/styles.rs` fully migrated to theme references
- [ ] All existing settings functionality preserved (tab switching, editing, saving)

### Phase 5 Complete When:
- [ ] All remaining widgets use Cyber-Glass theme
- [ ] Responsive layouts work at all supported terminal sizes
- [ ] All tests pass (including updated snapshots)
- [ ] No hardcoded colors remain outside `theme/` module

---

## Task Dependency Graph

```
Phase 1 (Foundation)
├── 01-create-theme-module (palette + icons + styles)
├── 02-create-modal-overlay-utils (dim, shadow, centering)
├── 03-migrate-widget-styles (all 15+ files → theme refs)
│   └── depends on: 01
├── 04-consolidate-phase-mapping (deduplicate 5 phase mappings)
│   └── depends on: 01
└── 05-update-tests-phase1 (fix broken tests from migration)
    └── depends on: 03, 04

Phase 2 (Main Log Screen) — depends on Phase 1
├── 06-redesign-header (glass container, device pill, shortcuts)
├── 07-redesign-log-view (glass container, metadata bars, styled entries)
├── 08-redesign-status-bar (merge into log footer or thin bar)
├── 09-update-layout (new proportions, gaps)
│   └── depends on: 06, 07, 08
└── 10-update-tests-phase2
    └── depends on: 06, 07, 08, 09

Phase 3 (New Session Modal) — depends on Phase 1
├── 11-redesign-modal-frame (overlay + shadow + glass frame)
│   └── depends on: 02
├── 12-redesign-target-selector (tab toggle, categorized devices)
├── 13-redesign-launch-context (fields, mode buttons, launch button)
├── 14-redesign-modal-footer (kbd-style shortcut hints)
└── 15-update-tests-phase3
    └── depends on: 11, 12, 13, 14

Phase 4 (Settings Panel) — depends on Phase 1
├── 16-redesign-settings-header (icon, title, pill tabs, esc hint)
├── 17-redesign-settings-content (group headers, 3-col setting rows, selected indicator)
├── 18-redesign-settings-special-views (user info banner, launch empty state)
├── 19-redesign-settings-footer (themed shortcut hints with icons)
├── 20-migrate-settings-styles (settings_panel/styles.rs → theme refs)
│   └── depends on: 01
├── 21-add-settings-icons (new icon constants + ASCII fallbacks)
│   └── depends on: 01
└── 22-update-tests-phase4
    └── depends on: 16, 17, 18, 19, 20

Phase 5 (Polish) — depends on Phase 2, Phase 3, Phase 4
├── 23-redesign-confirm-dialog
├── 24-redesign-search-input
├── 25-redesign-loading-screen
├── 26-redesign-project-selector
├── 27-update-nested-modals (fuzzy, dart-defines)
├── 28-responsive-testing
│   └── depends on: 23-27
└── 29-final-test-update
    └── depends on: 28
```

---

## Future Enhancements

- **User-configurable themes**: The centralized `theme/` module enables adding theme switching (dark, light, custom) via config
- **Terminal capability detection**: Auto-detect 256-color vs true-color and select appropriate palette
- **Animation system**: Tick-based animation for pulsing dots, progress indicators

---

## References

- Design reference: `tmp/redesign/logs-and-launch-modal-focus.tsx`
- Settings design reference: `tmp/redesign/settings-page-focus.tsx`
- [Ratatui Block widget docs](https://ratatui.rs/recipes/widgets/block/)
- [Ratatui border symbols](https://docs.rs/ratatui/latest/ratatui/symbols/border/index.html)
- [Ratatui styling text](https://ratatui.rs/recipes/render/style-text/)
- [Ratatui rendering under the hood](https://ratatui.rs/concepts/rendering/under-the-hood/)
- [Nerd Fonts cheat sheet](https://www.nerdfonts.com/cheat-sheet)
