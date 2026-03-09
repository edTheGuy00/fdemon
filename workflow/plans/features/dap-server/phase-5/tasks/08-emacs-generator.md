## Task: Emacs Config Generator

**Objective**: Implement the Emacs DAP config generator that produces a `.fdemon/dap-emacs.el` Elisp snippet for `dap-mode` integration, with instructions for the user to load it.

**Depends on**: 01-extend-parent-ide, 02-ide-config-trait

**Estimated Time**: 2–3 hours

### Scope

- `crates/fdemon-app/src/ide_config/emacs.rs`: **CREATE** — `EmacsGenerator` struct implementing `IdeConfigGenerator` with Elisp snippet generation
- `crates/fdemon-app/src/ide_config/mod.rs`: Add `pub mod emacs;` declaration

### Details

#### 1. Emacs DAP model

Emacs does not support project-local DAP configuration in a standard way. The `dap-mode` package requires explicit registration of debug providers and templates in the user's Emacs config.

The approach is "generate and instruct":
- Generate a `.fdemon/dap-emacs.el` file in the project root
- The file contains `dap-register-debug-provider` and `dap-register-debug-template` calls
- The user must manually load this file (`M-x load-file` or add to their config)
- fdemon logs instructions on how to load it

Since `.fdemon/` is fdemon's own directory, this file is always overwritten (not merged). No merge logic is needed.

#### 2. Generated Elisp content

```elisp
;; fdemon DAP configuration for Emacs dap-mode (auto-generated)
;;
;; Load this file to register fdemon as a DAP provider:
;;
;;   M-x load-file RET /path/to/project/.fdemon/dap-emacs.el RET
;;
;; Or add to your Emacs config:
;;
;;   (load-file "/path/to/project/.fdemon/dap-emacs.el")

(require 'dap-mode)

(dap-register-debug-provider
  "fdemon"
  (lambda (conf)
    (plist-put conf :debugPort 4711)
    (plist-put conf :host "localhost")
    conf))

(dap-register-debug-template
  "Flutter :: fdemon"
  (list :type "fdemon"
        :request "attach"
        :name "Flutter (fdemon DAP)"))
```

The port number (`4711` in the example) is substituted with the actual DAP server port.

#### 3. Trait implementation

```rust
pub struct EmacsGenerator;

impl IdeConfigGenerator for EmacsGenerator {
    fn config_path(&self, project_root: &Path) -> PathBuf {
        project_root.join(".fdemon").join("dap-emacs.el")
    }

    fn generate(&self, port: u16, project_root: &Path) -> crate::Result<String> {
        let path = self.config_path(project_root);
        Ok(self.generate_elisp(port, &path))
    }

    fn merge_config(&self, _existing: &str, port: u16) -> crate::Result<String> {
        // Emacs config is always overwritten, not merged.
        // The config_path is in .fdemon/ which is fdemon-owned.
        // We don't need project_root here since we're just generating content.
        // Use a placeholder path in the comments.
        Ok(self.generate_elisp_content_only(port))
    }

    fn ide_name(&self) -> &'static str {
        "Emacs"
    }
}
```

Since `.fdemon/dap-emacs.el` is fdemon-owned, the `merge_config` method simply regenerates the entire file. The dispatch function in `mod.rs` will call `merge_config` when the file exists, which effectively overwrites it — this is the correct behavior.

#### 4. Path in comments

The generated Elisp includes the full file path in the loading instructions so users can copy-paste:

```elisp
;; M-x load-file RET /absolute/path/to/project/.fdemon/dap-emacs.el RET
```

This requires the absolute path, which is derived from `project_root`. The `generate()` method receives `project_root` and can compute the absolute path. The `merge_config()` method doesn't receive `project_root`, so the instructions in the merge path may need to use a relative path or a placeholder.

### Acceptance Criteria

1. `config_path()` returns `.fdemon/dap-emacs.el`
2. Generated Elisp contains `dap-register-debug-provider` with correct port
3. Generated Elisp contains `dap-register-debug-template` with `request: "attach"`
4. Port number is correctly substituted in `:debugPort` field
5. File includes loading instructions in comments
6. `merge_config()` regenerates the file (overwrite semantics)
7. Generated Elisp is syntactically valid (parentheses balanced, strings quoted)
8. `.fdemon/` directory is created if it doesn't exist (handled by dispatch function)
9. `cargo check --workspace` — Pass
10. `cargo test -p fdemon-app` — Pass
11. `cargo clippy --workspace -- -D warnings` — Pass

### Testing

```rust
#[test]
fn test_emacs_config_path() {
    let gen = EmacsGenerator;
    assert_eq!(
        gen.config_path(Path::new("/project")),
        PathBuf::from("/project/.fdemon/dap-emacs.el")
    );
}

#[test]
fn test_emacs_fresh_generation() {
    let gen = EmacsGenerator;
    let content = gen.generate(4711, Path::new("/project")).unwrap();
    assert!(content.contains("dap-register-debug-provider"));
    assert!(content.contains(":debugPort 4711"));
    assert!(content.contains("dap-register-debug-template"));
    assert!(content.contains(":request \"attach\""));
    assert!(content.contains("require 'dap-mode"));
}

#[test]
fn test_emacs_port_substitution() {
    let gen = EmacsGenerator;
    let content = gen.generate(9999, Path::new("/project")).unwrap();
    assert!(content.contains(":debugPort 9999"));
    assert!(!content.contains(":debugPort 4711"));
}

#[test]
fn test_emacs_merge_overwrites() {
    let gen = EmacsGenerator;
    let old_content = ";; old content";
    let new_content = gen.merge_config(old_content, 5678).unwrap();
    assert!(new_content.contains(":debugPort 5678"));
    assert!(!new_content.contains("old content"));
}

#[test]
fn test_emacs_includes_loading_instructions() {
    let gen = EmacsGenerator;
    let content = gen.generate(4711, Path::new("/project")).unwrap();
    assert!(content.contains("load-file"));
    assert!(content.contains("M-x"));
}

#[test]
fn test_emacs_ide_name() {
    assert_eq!(EmacsGenerator.ide_name(), "Emacs");
}

#[test]
fn test_emacs_elisp_parens_balanced() {
    let gen = EmacsGenerator;
    let content = gen.generate(4711, Path::new("/project")).unwrap();
    // Simple paren balance check (ignoring strings/comments)
    let open = content.chars().filter(|c| *c == '(').count();
    let close = content.chars().filter(|c| *c == ')').count();
    assert_eq!(open, close, "Unbalanced parentheses in generated Elisp");
}
```

### Notes

- This is the simplest generator since it always overwrites (no merge complexity) and the target file is in fdemon's own `.fdemon/` directory.
- The Elisp snippet assumes `dap-mode` is installed. If it's not, the `(require 'dap-mode)` call will error. This is expected — users who run Emacs with dap-mode will have it installed.
- The `host` is hardcoded to `"localhost"` in the Elisp. For non-localhost bind addresses (a future enhancement), this would need to be parameterized.
- Emacs detection via `$INSIDE_EMACS` is not 100% reliable (some custom shell setups don't set it). Users can always fall back to `fdemon --dap-config emacs`.

---

## Completion Summary

**Status:** Done

### Files Modified

| File | Changes |
|------|---------|
| `crates/fdemon-app/src/ide_config/emacs.rs` | Created — `EmacsGenerator` struct implementing `IdeConfigGenerator`; `generate_elisp()` private helper; 10 unit tests |
| `crates/fdemon-app/src/ide_config/mod.rs` | Added `pub mod emacs;` declaration; added `run_generator()` file-I/O helper; wired `ParentIde::Emacs` arm in `generate_ide_config()` dispatch to `run_generator(&emacs::EmacsGenerator, ...)` |

### Notable Decisions/Tradeoffs

1. **`run_generator` helper in mod.rs**: The task description states the dispatch function owns file I/O. I added a private `run_generator()` function before the dispatch to handle mkdir/read/write, keeping all generators pure. The Helix task (Task 06) had already added this same function to mod.rs, so the Emacs arm just reuses it. No duplication was introduced.

2. **`generate_elisp` private function**: Content generation is extracted to a free function `generate_elisp(port, file_path_display)` rather than an `impl` method, keeping the `EmacsGenerator` struct minimal and making the format string easy to read and test in isolation.

3. **`merge_config` uses placeholder path**: Since `merge_config` doesn't receive `project_root`, the loading instructions in the overwritten file use `.fdemon/dap-emacs.el` as a relative placeholder. The `generate()` path embeds the full absolute path. This is an acceptable tradeoff documented in the task.

4. **Parenthesis balance verified**: The Elisp template contains `(fdemon DAP)` inside a string, which adds one `(` and one `)` that cancel out. The paren-balance test passes because the raw character counts are equal.

### Testing Performed

- `cargo check --workspace` — Passed
- `cargo test -p fdemon-app ide_config::emacs` — Passed (10/10 tests)
- `cargo test -p fdemon-app` — Passed (1425 tests, 0 failures)
- `cargo clippy --workspace -- -D warnings` — Passed (no warnings)
- `cargo fmt --all -- --check` — Passed

### Risks/Limitations

1. **No integration test for file write**: The `run_generator` helper that actually writes to disk is only exercised via `generate_ide_config(Some(ParentIde::Emacs), ...)` calls. Such a test would need a tempdir. The unit tests for `EmacsGenerator` itself are pure (no I/O), which satisfies the task's testing requirements.
