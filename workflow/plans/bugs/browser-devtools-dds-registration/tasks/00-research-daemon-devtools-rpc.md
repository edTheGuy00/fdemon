## Task: Research Flutter Daemon DevTools RPC Contract

**Agent:** external_researcher

**Objective**: Confirm the exact JSON-RPC contract for serving DevTools via the Flutter daemon, across SDK versions, before implementing.

**Depends on**: None

**Estimated Time**: 1-2 hours

### Scope

**Files Modified (Write):**
- `workflow/plans/bugs/browser-devtools-dds-registration/RESEARCH.md`: New research output document.

**Files Read (Dependencies):**
- None — pure external research.

### Research Goals

Answer these questions definitively, citing source URLs:

1. **Method name** — Is it `daemon.devtools.serve`, `daemon.serveDevTools`, or something else? Verify against the Flutter SDK's `packages/flutter_tools/lib/src/commands/daemon.dart` (or equivalent in current SDK).
2. **Request `params` shape** — Empty `{}`, or does it take `host`/`port`/`reuse` hints?
3. **Response `result` shape** — Likely `{ host: String, port: u16 }`; may also include `pid`. Confirm the exact key names.
4. **`daemon.devtools` event** — Is there a separate event emitted asynchronously, or is the response the only delivery channel? If both, when is each emitted?
5. **Minimum SDK version** — Earliest Flutter version that exposes the method. Verify on the Flutter changelog or `flutter_tools` git history.
6. **Failure mode on older SDKs** — Does the daemon return `-32601 Method not found`, or silent? Confirm via Dart Code / IntelliJ Flutter plugin source — they handle the same case.
7. **Auth tokens** — Does the served URL require any auth-token query parameter to bypass DDS auth? If so, where does it come from?
8. **`/inspector` deep-link** — Can the URL include a path like `/inspector` to land directly in the Inspector panel? Document the path conventions.
9. **Web Flutter** — How does this interact with web-target apps that emit `app.webLaunchUrl`?
10. **Sample wire bytes** — Capture a real example of the request, response, and event JSON from any of: Flutter source tests, IDE plugin tests, GitHub issues showing wire traces.

### Suggested Sources

- Flutter source: https://github.com/flutter/flutter/tree/master/packages/flutter_tools/lib/src/commands/daemon.dart
- Flutter daemon protocol docs: https://github.com/flutter/flutter/wiki/The-flutter-daemon-mode
- Dart Code (VS Code Flutter extension) — `daemon` handling: https://github.com/Dart-Code/Dart-Code
- IntelliJ Flutter plugin: https://github.com/flutter/flutter-intellij
- DDS source: https://github.com/dart-lang/sdk — `pkg/dds/`

### Deliverable

Write `workflow/plans/bugs/browser-devtools-dds-registration/RESEARCH.md` with sections:

```markdown
# Flutter Daemon DevTools RPC — Research Findings

## Verified Method Name
<method name + source URL>

## Request Schema
<JSON example + source>

## Response Schema
<JSON example + source>

## Event Schema (daemon.devtools)
<JSON example + source>

## Minimum SDK Version
<version + source>

## Failure Mode on Older SDKs
<behavior + source>

## URL Construction
<final URL shape + auth-token handling>

## Deep-link Paths
<inspector / performance / etc.>

## Web Flutter Note
<applicability + alternative for web targets>

## Sample Wire Traces
<actual JSON wire bytes from real sources>

## Recommendations for Implementor
<2-4 bullets specifically about what to do given the findings>
```

### Acceptance Criteria

1. RESEARCH.md exists, contains all 10 sections above.
2. Every factual claim has at least one source URL (Flutter source, IDE plugin source, or upstream issue).
3. At least one sample wire trace (request + response JSON) is included verbatim from a real source.
4. Recommendations section gives the implementor a clear go/no-go decision tree:
   - "If the method exists → use it; otherwise → fallback to legacy URL with toast."
   - "Minimum SDK is X; document fdemon's stance on older versions."

### Notes

- This task does NOT touch source code.
- If a key question cannot be answered from public sources, document the uncertainty and recommend the safest assumption.
- A 1-paragraph "stay tuned" summary at the top of RESEARCH.md is welcome — give the implementor the punchline before the details.
