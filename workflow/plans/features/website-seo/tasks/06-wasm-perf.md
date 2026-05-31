## Task: WASM bundle optimization (Cargo / Trunk)

**Objective**: Shrink the WASM payload to improve LCP/INP (Core Web Vitals = ranking
signal). nginx-side compression is handled in S08.

**Depends on**: None

**Agent:** implementor

**Estimated Time**: 1 hour

### Scope

**Files Modified (Write):**
- `website/Cargo.toml`: size-optimized release profile.
- `website/Trunk.toml`: confirm `wasm-opt` and `filehash`.

**Files Read (Dependencies):**
- Leptos binary-size guide.

### Details

- `website/Cargo.toml`: add a size-optimized release profile, e.g.
  ```toml
  [profile.release]
  opt-level = "z"
  lto = true
  codegen-units = 1
  panic = "abort"   # only if compatible with console_error_panic_hook usage
  ```
  (Validate `panic = "abort"` against the panic-hook dependency before enabling.)
- `website/Trunk.toml`: confirm `data-wasm-opt="z"` is applied (already set on the
  `rel="rust"` link in `index.html`); ensure `filehash = true` stays for cache-busting.
- Record before/after `.wasm` size in the PR notes.

### Acceptance Criteria

1. Release profile present; `trunk build --release` succeeds.
2. `.wasm` size reduced vs baseline (number recorded).
3. Site still loads and routes work.

### Notes

- Compression (gzip/brotli for `application/wasm`) is configured in S08 (nginx).
</content>
