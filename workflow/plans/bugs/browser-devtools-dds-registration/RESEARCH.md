# Flutter Daemon DevTools RPC — Research Findings

**Executive Summary**: Two assumptions in BUG.md are incorrect and must be fixed before coding. (1) The JSON-RPC method name is `"devtools.serve"` — NOT `"daemon.devtools.serve"`. Flutter daemon uses two-part `domain.method` notation: domain `devtools`, handler `serve`. (2) There is NO `"daemon.devtools"` event — that event name came from a superseded PR; the current `DevToolsDomain` emits no events at all. The async channel is `"app.devTools"` (fired automatically by `AppDomain` during `flutter run --machine` startup), which carries the base DevTools URL. Final URL for browser: `<event_uri>?uri=<percent_encoded_ws_uri>`. Minimum SDK: Flutter ~1.22.0 (Oct 2020). Older SDKs return `-32601 Method not found` on `devtools.serve`.

## Verified Method Name

**Method string**: `devtools.serve`

The daemon registers `DevToolsDomain` with domain name `'devtools'` and a single handler `'serve'`. In JSON-RPC wire format the method field is `"devtools.serve"`.

```dart
// From packages/flutter_tools/lib/src/commands/daemon.dart
class DevToolsDomain extends Domain {
  DevToolsDomain(Daemon daemon) : super(daemon, 'devtools') {
    registerHandler('serve', serve);
  }
}
```

Source: https://github.com/flutter/flutter/blob/master/packages/flutter_tools/lib/src/commands/daemon.dart

**CORRECTION TO BUG.MD AND TASK 01**: BUG.md phase 1 step 2 and task 01 reference `"daemon.devtools.serve"` as the method string. The correct JSON-RPC method string is `"devtools.serve"`. The Flutter daemon always uses two-part `domain.method` naming — there is no three-part form.

Note: `devtools.serve` is available in full `flutter daemon` mode. It is NOT documented in the restricted subset for `flutter run --machine` (the mode fdemon uses), though the `app.devTools` event (see Section 4) provides the same information automatically in `--machine` mode. See Recommendations.

## Request Schema

The `serve` handler accepts an optional map argument but ignores all parameters. Send an empty or omitted params:

```json
[{"id": "devtools-serve-1", "method": "devtools.serve"}]
```

With explicit empty params (also valid):
```json
[{"id": "devtools-serve-1", "method": "devtools.serve", "params": {}}]
```

**Wire framing note**: All Flutter daemon messages are wrapped in square brackets (`[{...}]`) as a single line. This is flutter-daemon-specific framing — NOT standard JSON-RPC 2.0.

Source: https://github.com/flutter/flutter/blob/master/packages/flutter_tools/doc/daemon.md

## Response Schema

The response is a synchronous JSON-RPC result. There is **no async event** from the DevTools domain. The `result` object contains only `host` and `port`. The `daemon.md` documentation mentions a `success` field but the Dart source code does not include one — treat null `host`/`port` as the failure indicator.

**Success (DevTools server started)**:
```json
[{"id": "devtools-serve-1", "result": {"host": "127.0.0.1", "port": 9100}}]
```

**Soft failure (DevTools launcher returned null server)**:
```json
[{"id": "devtools-serve-1", "result": {"host": null, "port": null}}]
```

**Hard failure (method not found on old SDK)**:
```json
[{"id": "devtools-serve-1", "error": {"code": -32601, "message": "Method not found"}}]
```

Source — complete `serve()` implementation:
```dart
Future<Map<String, Object?>> serve([Map<String, Object?>? args]) async {
  _devtoolsLauncher ??= DevtoolsLauncher.instance;
  final DevToolsServerAddress? server = await _devtoolsLauncher?.serve();
  return <String, Object?>{'host': server?.host, 'port': server?.port};
}
```

Source: https://github.com/flutter/flutter/blob/master/packages/flutter_tools/lib/src/commands/daemon.dart

Known issue with null results: https://github.com/flutter/flutter/issues/85705

**CORRECTION TO BUG.MD**: daemon.md documents a `success` boolean but the Dart source does not return one. Do not check `result.success`. Check for null `host`/`port` and for JSON-RPC `error` instead.

## Event Schema (app.devTools)

**CORRECTION TO BUG.MD**: The event `"daemon.devtools"` referenced in BUG.md does not exist in the current Flutter SDK. PR #62608 (Jul 31, 2020) originally emitted an event named `devtools.serve` during the serve operation, but PR #62702 (Aug 3, 2020) immediately superseded it to use a synchronous response with no event. The `DevToolsDomain.serve()` method makes no `sendEvent()` call — it only returns a response map.

The correct async channel is **`app.devTools`**, an app domain event fired automatically by `AppDomain.launch()` when DDS has DevTools ready:

```json
[{
  "event": "app.devTools",
  "params": {
    "appId": "8e5e5e3c-f5a3-4b6e-b3d1-123456789abc",
    "uri": "http://127.0.0.1:9100"
  }
}]
```

The `uri` field is the **base DevTools server URL** without any `?uri=` query parameter. Despite daemon.md saying "with query parameters already set to connect to the running application," the Dart source sends `info.devToolsUri!.toString()` which is `dds.devToolsUri` — the raw base URL from DDS. The implementor must append `?uri=<encoded_ws_uri>` manually.

Source — Dart source of the event emission:
```dart
// In AppDomain.launch():
if (info.devToolsUri != null) {
  _sendAppEvent(app, 'devTools', {'uri': info.devToolsUri!.toString()});
}

// _sendAppEvent adds appId automatically:
void _sendAppEvent(AppInstance app, String name, [Map<String, Object?>? args]) {
    sendEvent('app.$name', <String, Object?>{'appId': app.id, ...?args});
}
```

Source: https://github.com/flutter/flutter/blob/master/packages/flutter_tools/lib/src/commands/daemon.dart

**Documentation gap**: `app.devTools` is NOT listed in the `flutter run --machine` restricted subset in daemon.md, but the code sends it unconditionally regardless of mode. It fires in practice during `flutter run --machine` when DDS has `enableDevTools: true` (the default).

Source (enableDevTools default): https://github.com/flutter/flutter/blob/master/packages/flutter_tools/lib/src/base/dds.dart — method parameter `bool enableDevTools = true`

## Minimum SDK Version

**Flutter ~1.22.0** (stable, October 2020).

- PR #62608 ("Add daemon handler to start devtools") merged to `flutter:master` on **July 31, 2020**
- PR #62702 ("Return devtools serve response instead of printing") merged on **August 3, 2020**
- Flutter 1.22.0 stable released **October 2020** was the first stable release after these changes

Source: https://github.com/flutter/flutter/pull/62608 (merge date visible in PR metadata)
Source: https://github.com/flutter/flutter/pull/62702 (merge date)
Source: https://docs.flutter.dev/release/release-notes/release-notes-1.22.0

fdemon should document Flutter ≥ 1.22.0 as the minimum version for the DevTools browser feature. Users on older SDKs must use `dart devtools` manually and paste the VM Service URI.

Note: Flutter 3.27.0 (PR #152386) introduced DDS-launched DevTools, which may affect URI format on newest SDKs. The `devtools.serve` RPC remains stable since 1.22.0.

## Failure Mode on Older SDKs

On Flutter SDK versions before ~1.22.0, `devtools.serve` is not registered and returns:

```json
[{"id": "devtools-serve-1", "error": {"code": -32601, "message": "Method not found"}}]
```

`-32601` is the standard JSON-RPC 2.0 "Method not found" error code. This is the expected failure mode for unregistered daemon methods.

Source: https://github.com/flutter/flutter/issues/17335 — confirms `-32601` is used for unregistered flutter daemon methods

**Soft failure** (DevTools server unavailable but method exists):
- Response has `{"host": null, "port": null}` in the result
- Not a JSON-RPC error, just null fields
- Treat null host or port as failure

**Recommended handling**:
```
devtools.serve response:
  error.code == -32601  → log warn "devtools.serve unsupported (Flutter < 1.22)"; fallback to legacy URL; toast
  result.host == null   → log warn "DevTools server unavailable"; fallback; toast
  result.host != null   → store endpoint; use for browser open
```

## URL Construction

### Base URL from app.devTools Event

The `app.devTools` event's `uri` field contains the base DevTools server URL. Two formats exist:

**Standalone DevTools** (older Flutter / `dart devtools` invoked separately):
```
http://127.0.0.1:9100
```

**DDS-integrated DevTools** (newer Flutter, ~3.24+, DDS serves DevTools):
```
http://127.0.0.1:<dds-port>/<auth-token>/devtools
```
Example: `http://127.0.0.1:59123/tbrR0DzW2j8=/devtools`

Source: https://chromium.googlesource.com/external/github.com/flutter/engine/+/refs/tags/3.24.0-0.1.pre/docs/Using-the-Dart-Development-Service-(DDS)-and-Flutter-DevTools-with-a-custom-Flutter-Engine-Embedding.md

### Auth Token Handling

Auth tokens are embedded as **URL path segments** in the DDS base URI (e.g., `/tbrR0DzW2j8=/`). They are automatically included in the `app.devTools` event's `uri` field and in `devtools.serve` response's host+port combination. fdemon does NOT need to manage auth tokens separately — just use the URI from the event as-is for the base URL.

Source: https://chromium.googlesource.com/external/github.com/flutter/engine/+/refs/tags/3.24.0-0.1.pre/docs/Using-the-Dart-Development-Service-(DDS)-and-Flutter-DevTools-with-a-custom-Flutter-Engine-Embedding.md

### Final Browser URL

Construct by appending `?uri=` to the base URL:

```
<base_devtools_url>?uri=<percent_encoded_vm_service_ws_uri>
```

Examples:
```
http://127.0.0.1:9100?uri=ws%3A%2F%2F127.0.0.1%3A51830%2Fu37pq71Re0k%3D%2Fws
http://127.0.0.1:59123/tbrR0DzW2j8=/devtools?uri=ws%3A%2F%2F127.0.0.1%3A59123%2FtbrR0DzW2j8%3D%2Fws
```

Source: https://docs.flutter.dev/tools/devtools/cli — official docs show `http://127.0.0.1:9100?uri=http://127.0.0.1:51830/u37pq71Re0k=/`

**Note**: The existing `build_local_devtools_url` in fdemon (which constructs `http://<host>/<auth-token>/devtools/?uri=...`) targets the DDS `/devtools/` path which requires DevTools to be registered with DDS. Using the `app.devTools` event URI avoids this requirement as it points to the already-registered DevTools server.

## Deep-link Paths

DevTools uses hash-based routing. Available tab paths:

| Tab | Fragment |
|-----|---------|
| Inspector | `/#/inspector` |
| Performance | `/#/performance` |
| Network | `/#/network` |
| Memory | `/#/memory` |
| Debugger | `/#/debugger` |
| Logging | `/#/logging` |
| App Size | `/#/app-size` |

Screen IDs come from `ScreenMetaData.<name>.id` which conventionally uses lowercase tab names.

Source: https://api.flutter.dev/flutter/widgets/WidgetInspectorService/devToolsInspectorUri.html — confirms `/#/inspector` format
Source: https://github.com/flutter/devtools/issues/2475 — confirms hash-fragment routing behavior

**URL parameter ordering**: Query parameters (`?uri=...`) come BEFORE the hash fragment (`#/inspector`):
```
http://127.0.0.1:9100?uri=<encoded>#/inspector
```

Do NOT use standard `Uri.replace(fragment:)` — it encodes the `#` character, which breaks DevTools routing. Use string concatenation:
```rust
format!("{}?uri={}#{}", base_url, encoded_ws_uri, screen_path)
```

Source: https://api.flutter.dev/flutter/widgets/WidgetInspectorService/devToolsInspectorUri.html (implementation notes)

**Recommendation for this fix**: Open DevTools without a deep-link path (default tab). Deep-link support is a future enhancement.

## Web Flutter Note

**`app.devTools` event fires for web targets too.** The `app.devTools` event is platform-agnostic — it fires when DDS starts serving DevTools regardless of whether the Flutter app targets web or native platforms. fdemon should parse it for all target types.

**`app.webLaunchUrl` is unrelated.** This event fires when the Flutter web app's server is ready for browser access:

```json
[{
  "event": "app.webLaunchUrl",
  "params": {
    "appId": "<app-uuid>",
    "url": "http://localhost:55000",
    "launched": true
  }
}]
```

This gives the URL to the running web app, NOT to DevTools. The two events serve completely different purposes with no conflict.

Source: https://github.com/flutter/flutter/blob/master/packages/flutter_tools/doc/daemon.md

**`app.exposeUrl`** (enabled only with `--web-allow-expose-url`) is for URL mapping in remote environments. Out of scope for this fix.

## Sample Wire Traces

### Verbatim Wire Trace from PR #62608 (Jul 31, 2020)

This is the only public verbatim wire trace available. It shows the intermediate state before PR #62702 changed the response format. The `devtools.serve` EVENT shown no longer fires in current Flutter — it was replaced by a synchronous result. The request framing is still accurate.

```
[{"event":"daemon.connected","params":{"version":"0.6.0","pid":39909}}]
[{"event":"daemon.logMessage","params":{"level":"status","message":"Starting device daemon..."}}]
[{"id":"2","method":"devtools.serve"}]
Serving DevTools at http://127.0.0.1:55977
[{"event":"devtools.serve","params":{"host":"127.0.0.1","port":55977}}]
[{"id":"2"}]
```

Source: https://github.com/flutter/flutter/pull/62608 (PR description, verbatim)

**WARNING**: The `{"event":"devtools.serve",...}` line was removed by PR #62702. Current Flutter does NOT emit this event.

### Current Response Format (Post-PR #62702, Aug 3 2020 — present)

Inferred from Dart source code (DevToolsDomain.serve() implementation):

Request:
```json
[{"id":"devtools-serve-1","method":"devtools.serve"}]
```

Success response:
```json
[{"id":"devtools-serve-1","result":{"host":"127.0.0.1","port":9100}}]
```

Null/failure response (no DevTools server):
```json
[{"id":"devtools-serve-1","result":{"host":null,"port":null}}]
```

Method not found (pre-Flutter 1.22.0):
```json
[{"id":"devtools-serve-1","error":{"code":-32601,"message":"Method not found"}}]
```

Source: https://github.com/flutter/flutter/blob/master/packages/flutter_tools/lib/src/commands/daemon.dart

### app.devTools Event (fires automatically during flutter run --machine startup)

```json
[{
  "event": "app.devTools",
  "params": {
    "appId": "8e5e5e3c-f5a3-4b6e-b3d1-123456789abc",
    "uri": "http://127.0.0.1:9100"
  }
}]
```

Source: https://github.com/flutter/flutter/blob/master/packages/flutter_tools/lib/src/commands/daemon.dart (`_sendAppEvent(app, 'devTools', {'uri': info.devToolsUri!.toString()})`)

## Recommendations for Implementor

**Recommended implementation strategy:**

1. **Primary path — listen for `app.devTools` event** (no outgoing RPC needed):
   - In `protocol.rs`, add an arm for `event == "app.devTools"` that extracts `params.uri` string.
   - Create a new `DaemonMessage::DevToolsServed { app_id: String, base_url: String }` variant (note: the event carries a URL, not host+port — this differs from BUG.md's suggested `{host, port}` shape).
   - Store `session.devtools_url: Option<String>` (the base URL from the event).
   - When `B` is pressed: `format!("{}?uri={}", base_url, percent_encode(ws_uri))`.
   - This works on Flutter ≥ 1.22.0 without any outgoing command. No `DaemonCommand::ServeDevTools` variant is strictly required for the happy path, but adding it as an explicit fallback trigger is still valuable.

2. **Correct method name in all tasks** (tasks 01-03 reference wrong name):
   - The JSON-RPC `"method"` field must be `"devtools.serve"` — NOT `"daemon.devtools.serve"`.
   - Wire format: `[{"id":"<id>","method":"devtools.serve"}]`
   - If implementing `DaemonCommand::ServeDevTools` as a fallback trigger, serialize as `"devtools.serve"`.

3. **Decision tree for fallback:**
   - **If `app.devTools` event received** (happy path, Flutter ≥ 1.22.0): open `event.uri + "?uri=" + encode(ws_uri)`. No toast.
   - **If `app.devTools` not received and user presses `B`**: optionally call `devtools.serve` to try force-starting. If response returns non-null host+port, construct `http://<host>:<port>/?uri=<encoded>`.
   - **If `devtools.serve` returns null host/port or `-32601`**: fall back to legacy `build_local_devtools_url` with toast: "DevTools is not registered with DDS. Update Flutter to ≥ 1.22 or run `dart devtools` manually."
   - Minimum SDK for `devtools.serve` RPC: Flutter ~1.22.0 (Oct 2020). fdemon's stance: document this as minimum Flutter version for browser DevTools feature.

4. **Do NOT handle a `"daemon.devtools"` or `"devtools.serve"` event name** in `protocol.rs`. These event names do not exist in current Flutter SDK. The `app.devTools` APP event (event name `"app.devTools"`) is the correct target.

---

## Source URLs

- [Flutter daemon source (daemon.dart)](https://github.com/flutter/flutter/blob/master/packages/flutter_tools/lib/src/commands/daemon.dart)
- [Flutter daemon protocol docs (daemon.md)](https://github.com/flutter/flutter/blob/master/packages/flutter_tools/doc/daemon.md)
- [PR #62608 - Add daemon handler to start devtools](https://github.com/flutter/flutter/pull/62608)
- [PR #62702 - Return devtools serve response instead of printing](https://github.com/flutter/flutter/pull/62702)
- [Issue #85705 - devtools.serve returns null for host and port](https://github.com/flutter/flutter/issues/85705)
- [DDS and DevTools custom embedding guide](https://chromium.googlesource.com/external/github.com/flutter/engine/+/refs/tags/3.24.0-0.1.pre/docs/Using-the-Dart-Development-Service-(DDS)-and-Flutter-DevTools-with-a-custom-Flutter-Engine-Embedding.md)
- [Flutter 1.22.0 release notes](https://docs.flutter.dev/release/release-notes/release-notes-1.22.0)
- [Flutter 3.27.0 release notes](https://docs.flutter.dev/release/release-notes/release-notes-3.27.0)
- [WidgetInspectorService.devToolsInspectorUri](https://api.flutter.dev/flutter/widgets/WidgetInspectorService/devToolsInspectorUri.html)
- [Flutter DevTools CLI documentation](https://docs.flutter.dev/tools/devtools/cli)
- [DevTools URL fragment ordering issue #2475](https://github.com/flutter/devtools/issues/2475)
- [Flutter DDS source (dds.dart)](https://github.com/flutter/flutter/blob/master/packages/flutter_tools/lib/src/base/dds.dart)
