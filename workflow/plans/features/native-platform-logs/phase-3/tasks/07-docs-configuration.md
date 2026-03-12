## Task: Update docs/CONFIGURATION.md with Native Logs Section

**Objective**: Add a comprehensive `[native_logs]` configuration reference to `docs/CONFIGURATION.md`, covering all settings from phases 1-3 including custom sources.

**Depends on**: 04-app-custom-source-integration (implementation should be complete so docs match reality)

### Scope

- `docs/CONFIGURATION.md` — Add `[native_logs]` section

### Details

The file currently documents `[behavior]`, `[watcher]`, `[ui]`, `[devtools]`, and `[editor]` sections but has no `[native_logs]` section. Add it after `[devtools]` (or in a logical position).

#### Section Content

```markdown
## Native Logs

Native platform log capture settings. Controls how fdemon captures and displays native logs from Android (`adb logcat`), iOS (`idevicesyslog`/`simctl`), and macOS (`log stream`) alongside Flutter's Dart-level output.

### `[native_logs]`

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `enabled` | boolean | `true` | Master toggle for native log capture. When disabled, no native log processes are spawned. |
| `exclude_tags` | string array | `["flutter"]` | Tags to exclude from native capture. The `flutter` tag is excluded by default to avoid duplicating logs already captured via Flutter's `--machine` protocol. |
| `include_tags` | string array | `[]` | If non-empty, only show these tags (overrides `exclude_tags`). Acts as a whitelist. |
| `min_level` | string | `"info"` | Minimum log level for native logs. Options: `"verbose"`, `"debug"`, `"info"`, `"warning"`, `"error"`. |

#### Example

\```toml
[native_logs]
enabled = true
exclude_tags = ["flutter"]
min_level = "info"
\```

### `[native_logs.tags.<tag>]`

Per-tag level overrides. Applied before the UI-level tag filter (T key overlay).

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `min_level` | string | (inherits global) | Minimum log level for this specific tag. Overrides the global `min_level`. |

#### Example

\```toml
[native_logs.tags.GoLog]
min_level = "debug"

[native_logs.tags.OkHttp]
min_level = "warning"
\```

### `[[native_logs.custom_sources]]`

Define arbitrary log source processes. Each custom source spawns a command and parses its stdout as log entries. Custom sources run alongside the built-in platform capture.

| Setting | Type | Default | Description |
|---------|------|---------|-------------|
| `name` | string | (required) | Display name — becomes the tag in the log view and tag filter. |
| `command` | string | (required) | Path to the command to execute. |
| `args` | string array | `[]` | Command arguments. |
| `format` | string | `"raw"` | Output format parser. Options: `"raw"`, `"json"`, `"logcat-threadtime"`, `"syslog"`. |
| `working_dir` | string | (project dir) | Working directory for the command. |
| `env` | table | `{}` | Environment variables to set for the command. |

#### Format Options

- **`raw`**: Each line becomes a log message at Info level. The `name` is used as the tag.
- **`json`**: Expects JSON objects with fields: `message`/`msg`/`text`, `level`/`severity`/`priority`, `tag`/`source`/`logger`, `timestamp`/`time`/`ts`. Unknown fields ignored.
- **`logcat-threadtime`**: Android logcat `threadtime` format: `MM-DD HH:MM:SS.mmm PID TID PRIO TAG : message`.
- **`syslog`**: macOS/iOS unified logging compact format.

#### Examples

\```toml
# Tail a log file
[[native_logs.custom_sources]]
name = "sidecar"
command = "tail"
args = ["-f", "/tmp/my-app.log"]
format = "raw"

# JSON log stream
[[native_logs.custom_sources]]
name = "api-server"
command = "/usr/local/bin/my-log-tool"
args = ["--follow", "--json"]
format = "json"
env = { LOG_LEVEL = "debug" }

# Filtered Android logcat for a specific tag
[[native_logs.custom_sources]]
name = "go-backend"
command = "adb"
args = ["logcat", "GoLog:D", "*:S", "-v", "threadtime"]
format = "logcat-threadtime"
\```
```

**Note**: The `\```toml` above uses escaped backticks for the plan file — the actual markdown should use unescaped triple backticks.

### Acceptance Criteria

1. `docs/CONFIGURATION.md` has a complete `[native_logs]` section
2. All settings documented with types, defaults, and descriptions
3. Per-tag overrides documented with examples
4. Custom sources documented with all fields and format options
5. Format options explained with when to use each
6. Examples are valid TOML and match the actual config parsing

### Testing

- Verify all TOML examples in the docs parse correctly by testing against `NativeLogsSettings` deserialization
- No automated tests needed for this task (documentation only)

### Notes

- Follow the existing documentation style in `CONFIGURATION.md` — table format for settings, code blocks for examples
- Make sure the defaults documented here match the actual `Default` impl in `config/types.rs`
- Cross-reference the tag filter UI: mention the `T` key for runtime tag filtering alongside config-level filtering
