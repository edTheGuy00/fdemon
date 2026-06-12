//! Generic VM service-extension pass-through for external consumers
//!
//! This module provides the [`VmExtensionService`] trait so embedders (e.g. an
//! MCP server or an app-side driver package) can invoke arbitrary service
//! extensions registered on a session's Dart VM (`ext.flutter.*`,
//! `ext.dart.io.*`, or app-defined extensions like `ext.fdemon.driver.*`) and
//! discover which extensions are registered — without direct access to the
//! Engine and without any TUI interaction.
//!
//! ## How calls reach the VM
//!
//! Following the `vm_handle_for_dap` precedent: the Engine syncs each
//! session's [`VmRequestHandle`] into [`SharedState::vm_handles`] after every
//! TEA cycle. The service clones the handle for the requested session and
//! issues the RPC directly over the session's VM Service WebSocket — the same
//! machinery the inspector uses for `ext.flutter.inspector.*` calls. No TEA
//! round-trip is involved.
//!
//! Extension calls target the **Flutter UI isolate**, resolved (and cached)
//! via [`VmRequestHandle::resolve_flutter_ui_isolate`] — the isolate whose
//! `extensionRPCs` advertise `ext.flutter.*` extensions.
//!
//! ## Security note
//!
//! The service is intentionally generic: **no allowlist is enforced**.
//! Callers are responsible for what they invoke. Service extensions are only
//! served by debug/profile-mode VMs on a localhost-bound (auth-token
//! protected) WebSocket, and the TUI itself never calls this service — it
//! exists purely as an embedder seam.

use std::sync::Arc;
use std::time::Duration;

use fdemon_core::prelude::*;
use fdemon_daemon::vm_service::VmRequestHandle;

use super::state_service::SharedState;
use crate::session::SessionId;

/// Timeout applied to [`VmExtensionService::list_service_extensions`].
///
/// Listing is two lightweight RPCs (`getVM` + `getIsolate`); 5 seconds is
/// generous while still guaranteeing the call cannot hang on a wedged VM.
const LIST_EXTENSIONS_TIMEOUT: Duration = Duration::from_secs(5);

// ─────────────────────────────────────────────────────────────────────────────
// Service trait
// ─────────────────────────────────────────────────────────────────────────────

/// Per-session pass-through to registered Dart VM service extensions.
///
/// Both methods are headless-usable and work for **any** session (not only
/// the selected one), as long as that session's VM Service is connected.
///
/// **Callers are responsible for what they invoke** — see the module-level
/// security note.
#[trait_variant::make(VmExtensionService: Send)]
pub trait LocalVmExtensionService {
    /// Invoke a registered service extension on the session's Flutter UI
    /// isolate and return the raw JSON `result`.
    ///
    /// - `method` is the fully qualified extension name (e.g.
    ///   `"ext.fdemon.driver.tap"`). It is passed through verbatim — no
    ///   prefix validation is performed.
    /// - `args` must be a JSON object (its entries become RPC params next to
    ///   the auto-injected `isolateId`) or `null` for no arguments. Note the
    ///   VM Service delivers extension parameter values to Dart as strings;
    ///   non-string values are accepted here and serialized as-is, but most
    ///   extensions expect string values.
    /// - `timeout` bounds the whole operation, including isolate resolution.
    ///
    /// # Errors
    ///
    /// - [`Error::VmService`] when the session is unknown or its VM Service
    ///   is not connected, when `args` is neither an object nor `null`, when
    ///   the extension is not registered (VM RPC error `-32601`), or when
    ///   `timeout` elapses.
    /// - Any other VM RPC or transport error is passed through unchanged
    ///   (e.g. [`Error::Protocol`] for extension-side exceptions,
    ///   [`Error::ChannelClosed`] when the connection drops mid-call).
    async fn call_service_extension(
        &self,
        session_id: SessionId,
        method: &str,
        args: serde_json::Value,
        timeout: Duration,
    ) -> Result<serde_json::Value>;

    /// List the service extensions registered on the session's Flutter UI
    /// isolate (the isolate's advertised `extensionRPCs`).
    ///
    /// Queried live via `getIsolate` so the list reflects extensions
    /// registered after startup (e.g. by a deferred driver package). Bounded
    /// by an internal 5-second timeout.
    ///
    /// # Errors
    ///
    /// [`Error::VmService`] when the session is unknown, its VM Service is
    /// not connected, or the query times out; transport/RPC errors are
    /// passed through.
    async fn list_service_extensions(&self, session_id: SessionId) -> Result<Vec<String>>;
}

// ─────────────────────────────────────────────────────────────────────────────
// SharedState-backed implementation
// ─────────────────────────────────────────────────────────────────────────────

/// Implementation backed by the per-session VM handles the Engine syncs into
/// [`SharedState::vm_handles`]. Cheap to construct and `Send + 'static`, so
/// it can be moved into spawned tokio tasks.
pub struct SharedVmExtensionService {
    state: Arc<SharedState>,
}

impl SharedVmExtensionService {
    pub fn new(state: Arc<SharedState>) -> Self {
        Self { state }
    }

    /// Clone the synced VM handle for a session, erroring honestly when the
    /// session is unknown or its VM Service is not currently connected.
    async fn connected_handle(&self, session_id: SessionId) -> Result<VmRequestHandle> {
        let handle = self
            .state
            .vm_handles
            .read()
            .await
            .get(&session_id)
            .cloned()
            .ok_or_else(|| {
                Error::vm_service(format!(
                    "VM Service not connected for session {session_id} — the app must be \
                     running in debug or profile mode"
                ))
            })?;
        if !handle.is_connected() {
            return Err(Error::vm_service(format!(
                "VM Service for session {session_id} is currently disconnected (reconnecting)"
            )));
        }
        Ok(handle)
    }
}

/// Returns `true` if the error indicates "method not found" (extension not
/// registered). The VM Service error code `-32601` is embedded in the
/// [`Error::Protocol`] message by the client's error mapping.
fn is_method_not_found(error: &Error) -> bool {
    match error {
        Error::Protocol { message } => {
            message.contains("-32601") || message.to_lowercase().contains("method not found")
        }
        _ => false,
    }
}

/// Extract the advertised extension RPC names from a raw `getIsolate` reply.
///
/// Read from the raw JSON rather than the typed `IsolateInfo`: the VM Service
/// spec spells the field `extensionRPCs`, which `IsolateInfo`'s
/// `rename_all = "camelCase"` derive does not match (it expects
/// `extensionRpcs`). Both spellings are accepted here. A missing/absent field
/// yields an empty list (e.g. release mode, or an isolate with no extensions).
fn parse_extension_rpcs(get_isolate_result: &serde_json::Value) -> Vec<String> {
    get_isolate_result
        .get("extensionRPCs")
        .or_else(|| get_isolate_result.get("extensionRpcs"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default()
}

/// Merge user args into the RPC params object next to `isolateId`.
///
/// Errors when `args` is neither a JSON object nor `null`.
fn build_params(isolate_id: &str, args: serde_json::Value) -> Result<serde_json::Value> {
    let mut params = serde_json::Map::new();
    params.insert(
        "isolateId".to_string(),
        serde_json::Value::String(isolate_id.to_string()),
    );
    match args {
        serde_json::Value::Null => {}
        serde_json::Value::Object(map) => params.extend(map),
        other => {
            return Err(Error::vm_service(format!(
                "service extension args must be a JSON object or null, got: {other}"
            )));
        }
    }
    Ok(serde_json::Value::Object(params))
}

impl VmExtensionService for SharedVmExtensionService {
    async fn call_service_extension(
        &self,
        session_id: SessionId,
        method: &str,
        args: serde_json::Value,
        timeout: Duration,
    ) -> Result<serde_json::Value> {
        let handle = self.connected_handle(session_id).await?;

        let call = async {
            let isolate_id = handle.resolve_flutter_ui_isolate().await?;
            let params = build_params(&isolate_id, args)?;
            handle.request(method, Some(params)).await
        };

        match tokio::time::timeout(timeout, call).await {
            Err(_elapsed) => Err(Error::vm_service(format!(
                "service extension call '{method}' timed out after {timeout:?}"
            ))),
            Ok(Err(e)) if is_method_not_found(&e) => Err(Error::vm_service(format!(
                "service extension '{method}' is not registered on session {session_id}'s VM \
                 (RPC -32601) — check list_service_extensions for available methods"
            ))),
            Ok(result) => result,
        }
    }

    async fn list_service_extensions(&self, session_id: SessionId) -> Result<Vec<String>> {
        let handle = self.connected_handle(session_id).await?;

        let query = async {
            let isolate_id = handle.resolve_flutter_ui_isolate().await?;
            let result = handle
                .request(
                    "getIsolate",
                    Some(serde_json::json!({ "isolateId": isolate_id })),
                )
                .await?;
            Ok(parse_extension_rpcs(&result))
        };

        match tokio::time::timeout(LIST_EXTENSIONS_TIMEOUT, query).await {
            Err(_elapsed) => Err(Error::vm_service(format!(
                "listing service extensions for session {session_id} timed out after \
                 {LIST_EXTENSIONS_TIMEOUT:?}"
            ))),
            Ok(result) => result,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    // Import only the Send-variant trait: `SharedVmExtensionService`
    // implements both variants (Local via the trait_variant blanket impl), so
    // importing both would make plain method-call syntax ambiguous.
    use super::{SharedVmExtensionService, VmExtensionService};
    use crate::services::SharedState;
    use crate::session::SessionId;
    use fdemon_daemon::vm_service::client::ClientCommand;
    use fdemon_daemon::vm_service::VmRequestHandle;
    use serde_json::json;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::sync::mpsc;

    const SESSION: SessionId = 1;

    /// Service plus the command receiver backing session 1's VM handle.
    async fn create_service() -> (SharedVmExtensionService, mpsc::Receiver<ClientCommand>) {
        let state = Arc::new(SharedState::new(100));
        let (handle, cmd_rx) = VmRequestHandle::new_with_test_channel();
        state.vm_handles.write().await.insert(SESSION, handle);
        (SharedVmExtensionService::new(state), cmd_rx)
    }

    /// Reply for `getVM`: one non-system isolate.
    fn get_vm_response() -> serde_json::Value {
        json!({
            "name": "vm",
            "version": "3.4.0",
            "isolates": [
                { "id": "isolates/1", "name": "main", "number": "1", "isSystemIsolate": false }
            ]
        })
    }

    /// Reply for `getIsolate`: advertises one Flutter and one driver extension.
    fn get_isolate_response() -> serde_json::Value {
        json!({
            "id": "isolates/1",
            "name": "main",
            "extensionRPCs": ["ext.flutter.platformOverride", "ext.fdemon.driver.tap"]
        })
    }

    /// Drive a fake VM: answers `getVM`/`getIsolate`, delegates everything
    /// else (the extension call) to `on_extension`.
    fn spawn_fake_vm(
        mut cmd_rx: mpsc::Receiver<ClientCommand>,
        on_extension: impl Fn(&str, Option<serde_json::Value>) -> fdemon_core::Result<serde_json::Value>
            + Send
            + 'static,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            while let Some(cmd) = cmd_rx.recv().await {
                let ClientCommand::SendRequest {
                    method,
                    params,
                    response_tx,
                } = cmd
                else {
                    continue;
                };
                let reply = match method.as_str() {
                    "getVM" => Ok(get_vm_response()),
                    "getIsolate" => Ok(get_isolate_response()),
                    other => on_extension(other, params),
                };
                let _ = response_tx.send(reply);
            }
        })
    }

    #[tokio::test]
    async fn test_call_service_extension_unknown_session_returns_not_connected_error() {
        let state = Arc::new(SharedState::new(100));
        let service = SharedVmExtensionService::new(state);

        let err = service
            .call_service_extension(
                99,
                "ext.fdemon.driver.tap",
                json!(null),
                Duration::from_secs(1),
            )
            .await
            .unwrap_err();

        assert!(
            err.to_string().contains("not connected"),
            "expected not-connected error, got: {err}"
        );
    }

    #[tokio::test]
    async fn test_list_service_extensions_unknown_session_returns_not_connected_error() {
        let state = Arc::new(SharedState::new(100));
        let service = SharedVmExtensionService::new(state);

        let err = service.list_service_extensions(99).await.unwrap_err();
        assert!(
            err.to_string().contains("not connected"),
            "expected not-connected error, got: {err}"
        );
    }

    #[tokio::test]
    async fn test_call_service_extension_dispatches_rpc_and_returns_result() {
        let (service, cmd_rx) = create_service().await;
        let vm = spawn_fake_vm(cmd_rx, |method, params| {
            assert_eq!(method, "ext.fdemon.driver.tap");
            let params = params.expect("extension call must carry params");
            // isolateId is auto-injected; user args ride alongside it.
            assert_eq!(params["isolateId"], "isolates/1");
            assert_eq!(params["finder"], "byText('Login')");
            Ok(json!({ "status": "tapped" }))
        });

        let result = service
            .call_service_extension(
                SESSION,
                "ext.fdemon.driver.tap",
                json!({ "finder": "byText('Login')" }),
                Duration::from_secs(2),
            )
            .await
            .unwrap();

        assert_eq!(result["status"], "tapped");
        vm.abort();
    }

    #[tokio::test]
    async fn test_call_service_extension_null_args_sends_only_isolate_id() {
        let (service, cmd_rx) = create_service().await;
        let vm = spawn_fake_vm(cmd_rx, |_, params| {
            let params = params.expect("extension call must carry params");
            let obj = params.as_object().unwrap();
            assert_eq!(obj.len(), 1, "null args must add nothing beyond isolateId");
            assert_eq!(obj["isolateId"], "isolates/1");
            Ok(json!({}))
        });

        service
            .call_service_extension(
                SESSION,
                "ext.fdemon.driver.ping",
                json!(null),
                Duration::from_secs(2),
            )
            .await
            .unwrap();
        vm.abort();
    }

    #[tokio::test]
    async fn test_call_service_extension_rejects_non_object_args() {
        let (service, cmd_rx) = create_service().await;
        let vm = spawn_fake_vm(cmd_rx, |_, _| Ok(json!({})));

        let err = service
            .call_service_extension(
                SESSION,
                "ext.fdemon.driver.tap",
                json!([1, 2, 3]),
                Duration::from_secs(2),
            )
            .await
            .unwrap_err();

        assert!(
            err.to_string().contains("JSON object or null"),
            "expected args-shape error, got: {err}"
        );
        vm.abort();
    }

    #[tokio::test]
    async fn test_call_service_extension_unknown_method_reports_not_registered() {
        let (service, cmd_rx) = create_service().await;
        let vm = spawn_fake_vm(cmd_rx, |_, _| {
            Err(fdemon_core::Error::Protocol {
                message: "VM Service RPC error -32601: Method not found".to_string(),
            })
        });

        let err = service
            .call_service_extension(
                SESSION,
                "ext.fdemon.driver.doesNotExist",
                json!(null),
                Duration::from_secs(2),
            )
            .await
            .unwrap_err();

        let msg = err.to_string();
        assert!(
            msg.contains("not registered") && msg.contains("ext.fdemon.driver.doesNotExist"),
            "expected not-registered error naming the method, got: {msg}"
        );
        vm.abort();
    }

    #[tokio::test]
    async fn test_call_service_extension_passes_through_other_rpc_errors() {
        let (service, cmd_rx) = create_service().await;
        let vm = spawn_fake_vm(cmd_rx, |_, _| {
            Err(fdemon_core::Error::Protocol {
                message: "VM Service RPC error -32603: extension threw StateError".to_string(),
            })
        });

        let err = service
            .call_service_extension(
                SESSION,
                "ext.fdemon.driver.tap",
                json!(null),
                Duration::from_secs(2),
            )
            .await
            .unwrap_err();

        assert!(
            err.to_string().contains("-32603"),
            "non--32601 RPC errors must pass through unchanged, got: {err}"
        );
        vm.abort();
    }

    #[tokio::test]
    async fn test_call_service_extension_times_out_when_vm_never_replies() {
        let (service, mut cmd_rx) = create_service().await;
        // Answer isolate resolution, then hold the extension call's
        // response_tx without replying (dropping it would yield ChannelClosed
        // instead of a timeout).
        let vm = tokio::spawn(async move {
            let mut held = Vec::new();
            while let Some(ClientCommand::SendRequest {
                method,
                response_tx,
                ..
            }) = cmd_rx.recv().await
            {
                match method.as_str() {
                    "getVM" => {
                        let _ = response_tx.send(Ok(get_vm_response()));
                    }
                    "getIsolate" => {
                        let _ = response_tx.send(Ok(get_isolate_response()));
                    }
                    _ => held.push(response_tx),
                }
            }
        });

        let err = service
            .call_service_extension(
                SESSION,
                "ext.fdemon.driver.tap",
                json!(null),
                Duration::from_millis(150),
            )
            .await
            .unwrap_err();

        assert!(
            err.to_string().contains("timed out"),
            "expected timeout error, got: {err}"
        );
        vm.abort();
    }

    #[test]
    fn test_parse_extension_rpcs_accepts_spec_and_legacy_spellings() {
        let spec = json!({ "extensionRPCs": ["ext.a", "ext.b"] });
        assert_eq!(super::parse_extension_rpcs(&spec), vec!["ext.a", "ext.b"]);

        let legacy = json!({ "extensionRpcs": ["ext.c"] });
        assert_eq!(super::parse_extension_rpcs(&legacy), vec!["ext.c"]);
    }

    #[test]
    fn test_parse_extension_rpcs_missing_field_returns_empty() {
        let none = json!({ "id": "isolates/1", "name": "main" });
        assert!(super::parse_extension_rpcs(&none).is_empty());
    }

    #[tokio::test]
    async fn test_list_service_extensions_returns_advertised_rpcs() {
        let (service, cmd_rx) = create_service().await;
        let vm = spawn_fake_vm(cmd_rx, |_, _| Ok(json!({})));

        let extensions = service.list_service_extensions(SESSION).await.unwrap();

        assert_eq!(
            extensions,
            vec![
                "ext.flutter.platformOverride".to_string(),
                "ext.fdemon.driver.tap".to_string()
            ]
        );
        vm.abort();
    }

    #[tokio::test]
    async fn test_service_usable_from_spawned_task() {
        let (service, cmd_rx) = create_service().await;
        let vm = spawn_fake_vm(cmd_rx, |_, _| Ok(json!({ "ok": true })));

        let handle = tokio::spawn(async move {
            service
                .call_service_extension(
                    SESSION,
                    "ext.fdemon.driver.ping",
                    json!(null),
                    Duration::from_secs(2),
                )
                .await
        });

        let result = handle.await.unwrap().unwrap();
        assert_eq!(result["ok"], true);
        vm.abort();
    }
}
