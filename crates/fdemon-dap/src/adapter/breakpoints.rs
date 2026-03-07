//! # Breakpoint State
//!
//! Provides [`BreakpointState`] which tracks the mapping between DAP
//! breakpoint IDs (integers) and VM Service breakpoint IDs (strings), and
//! records resolution status for each breakpoint.
//!
//! Also provides [`BreakpointManager`] which separates "desired" breakpoint
//! state (what the IDE requested) from "active" VM-tracked state. The desired
//! state survives hot restart; only the active/VM state is invalidated.
//!
//! Breakpoints are set via `setBreakpoints` DAP requests (implemented in
//! Task 05). This module provides the scaffolding needed to track them.
//!
//! ## Conditional Breakpoints
//!
//! Each [`BreakpointEntry`] may carry an optional `condition` (a Dart
//! expression) and/or `hit_condition` (a simple operator expression like
//! `">= 3"` or `"% 2 == 0"`). The adapter evaluates these at pause time:
//!
//! - `hit_condition` is checked first (cheap — no VM RPC).
//! - `condition` is then evaluated via `evaluateInFrame` if the hit condition
//!   passes (or is absent).
//! - If all applicable conditions pass, the `stopped` event is emitted.
//! - Otherwise the isolate is silently resumed.
//!
//! ## Logpoints
//!
//! A breakpoint with a non-empty `log_message` is a *logpoint*. When it fires
//! and all conditions pass, the adapter interpolates `{expression}` placeholders
//! in the message template via `evaluateInFrame`, emits a DAP `output` event,
//! and auto-resumes the isolate. Execution is **not** suspended.
//!
//! Use [`parse_log_message`] to split a template string into [`LogSegment`]
//! pieces for interpolation.

use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// LogSegment / parse_log_message
// ─────────────────────────────────────────────────────────────────────────────

/// A segment of a logpoint message template.
///
/// The template `"x = {x}, y = {y}"` is parsed into:
/// ```text
/// [Literal("x = "), Expression("x"), Literal(", y = "), Expression("y")]
/// ```
///
/// See [`parse_log_message`] for details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LogSegment {
    /// A literal string fragment (no evaluation needed).
    Literal(String),
    /// A Dart expression to be evaluated via `evaluateInFrame`.
    Expression(String),
}

/// Parse a logpoint message template into a sequence of [`LogSegment`]s.
///
/// `{expression}` syntax is used to interpolate Dart expressions. Any `{` that
/// is not matched by a closing `}` is treated as a literal character (and the
/// rest of the string after the unmatched `{` is also treated as a literal).
///
/// The DAP spec does not define an escape mechanism for literal `{` in
/// logpoints — any `{` is interpreted as starting an expression.
///
/// # Examples
///
/// ```rust
/// # use fdemon_dap::adapter::breakpoints::{parse_log_message, LogSegment};
/// let segs = parse_log_message("x = {x}");
/// assert_eq!(segs.len(), 2);
/// assert!(matches!(&segs[0], LogSegment::Literal(s) if s == "x = "));
/// assert!(matches!(&segs[1], LogSegment::Expression(s) if s == "x"));
/// ```
pub fn parse_log_message(template: &str) -> Vec<LogSegment> {
    let mut segments = Vec::new();
    let mut remaining = template;

    while let Some(open) = remaining.find('{') {
        let literal = &remaining[..open];
        if let Some(close) = remaining[open..].find('}') {
            let expr = &remaining[open + 1..open + close];
            if !literal.is_empty() {
                segments.push(LogSegment::Literal(literal.to_string()));
            }
            segments.push(LogSegment::Expression(expr.to_string()));
            remaining = &remaining[open + close + 1..];
        } else {
            // Unmatched opening brace — treat the rest as a literal.
            break;
        }
    }

    // Any trailing text (or the whole string if no `{` was found).
    if !remaining.is_empty() {
        segments.push(LogSegment::Literal(remaining.to_string()));
    }

    segments
}

// ─────────────────────────────────────────────────────────────────────────────
// BreakpointCondition
// ─────────────────────────────────────────────────────────────────────────────

/// Condition configuration for a tracked breakpoint.
///
/// Passed to [`BreakpointState::add_with_condition`] to keep the argument
/// count manageable.
#[derive(Debug, Clone, Default)]
pub struct BreakpointCondition {
    /// An optional Dart expression that must evaluate to truthy for the
    /// breakpoint to pause execution.
    pub condition: Option<String>,
    /// An optional hit-count expression (e.g., `">= 3"`, `"% 2 == 0"`).
    pub hit_condition: Option<String>,
    /// An optional logpoint message template. When set, the breakpoint acts as
    /// a *logpoint*: the message is interpolated and emitted as a DAP `output`
    /// event, and execution is **not** suspended. `{expression}` syntax is used
    /// to embed Dart expressions in the template.
    pub log_message: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// BreakpointEntry
// ─────────────────────────────────────────────────────────────────────────────

/// A tracked breakpoint: DAP ID ↔ VM Service ID mapping plus resolution state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreakpointEntry {
    /// DAP breakpoint ID (assigned by the adapter, integer, 1-based).
    pub dap_id: i64,
    /// VM Service breakpoint ID (assigned by the Dart VM, opaque string).
    pub vm_id: String,
    /// The source URI this breakpoint was set in.
    pub uri: String,
    /// The requested line (1-based).
    pub line: Option<i32>,
    /// The requested column (1-based), if specified.
    pub column: Option<i32>,
    /// Whether the VM has confirmed this breakpoint is at a valid location.
    pub verified: bool,
    /// An optional Dart expression that must evaluate to truthy for the
    /// breakpoint to pause execution. `None` means unconditional.
    pub condition: Option<String>,
    /// An optional hit-condition expression (e.g., `">= 3"`, `"% 2 == 0"`).
    /// `None` means the breakpoint fires on every hit.
    pub hit_condition: Option<String>,
    /// Number of times this breakpoint has been hit (incremented on every VM
    /// `PauseBreakpoint` event for this breakpoint, before condition checks).
    pub hit_count: u64,
    /// An optional logpoint message template. When non-empty, this breakpoint
    /// is a *logpoint*: execution is not paused; instead the interpolated
    /// message is emitted as a DAP `output` event and the isolate auto-resumes.
    ///
    /// `None` means this is a regular (non-logpoint) breakpoint.
    pub log_message: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// BreakpointState
// ─────────────────────────────────────────────────────────────────────────────

/// Tracks all active breakpoints across sources.
///
/// Maintains two indices for O(1) lookup:
/// - DAP ID → entry (for client-initiated operations)
/// - VM ID → DAP ID (for VM-initiated resolution events)
///
/// # Lifecycle
///
/// When `setBreakpoints` is called for a source, the adapter first removes all
/// existing breakpoints for that source, then adds the new set. This replaces
/// the full set atomically from the client's perspective.
pub struct BreakpointState {
    /// All tracked breakpoints, keyed by DAP breakpoint ID.
    by_dap_id: HashMap<i64, BreakpointEntry>,
    /// Maps VM Service breakpoint ID → DAP breakpoint ID.
    vm_id_to_dap_id: HashMap<String, i64>,
    /// Next DAP breakpoint ID to assign (1-based, monotonically increasing).
    next_dap_id: i64,
}

impl BreakpointState {
    /// Create a new empty [`BreakpointState`].
    pub fn new() -> Self {
        Self {
            by_dap_id: HashMap::new(),
            vm_id_to_dap_id: HashMap::new(),
            next_dap_id: 1,
        }
    }

    /// Register a new unconditional breakpoint and return its assigned DAP ID.
    ///
    /// The `vm_id` is the ID returned by the VM Service after adding the
    /// breakpoint. The `line` and `column` are the actual location (which may
    /// differ from the requested location).
    ///
    /// For conditional breakpoints, use [`BreakpointState::add_with_condition`].
    pub fn add(
        &mut self,
        vm_id: impl Into<String>,
        uri: impl Into<String>,
        line: Option<i32>,
        column: Option<i32>,
        verified: bool,
    ) -> i64 {
        self.add_with_condition(
            vm_id,
            uri,
            line,
            column,
            verified,
            BreakpointCondition::default(),
        )
    }

    /// Register a new breakpoint with optional condition/hit-condition strings.
    ///
    /// Extends [`BreakpointState::add`] with support for conditional and
    /// hit-conditional breakpoints. The `condition` and `hit_condition` strings
    /// are stored verbatim and evaluated at pause time by the adapter.
    ///
    /// # Arguments
    ///
    /// * `vm_id` — VM Service breakpoint ID (opaque string).
    /// * `uri` — Source URI for the breakpoint.
    /// * `line` — Resolved source line (1-based), or `None` if not yet known.
    /// * `column` — Resolved source column (1-based), or `None`.
    /// * `verified` — Whether the VM has confirmed the breakpoint location.
    /// * `conds` — Optional condition configuration; use `Default::default()` for
    ///   an unconditional breakpoint.
    pub fn add_with_condition(
        &mut self,
        vm_id: impl Into<String>,
        uri: impl Into<String>,
        line: Option<i32>,
        column: Option<i32>,
        verified: bool,
        conds: BreakpointCondition,
    ) -> i64 {
        let dap_id = self.next_dap_id;
        self.next_dap_id += 1;
        let vm_id = vm_id.into();

        self.vm_id_to_dap_id.insert(vm_id.clone(), dap_id);
        self.by_dap_id.insert(
            dap_id,
            BreakpointEntry {
                dap_id,
                vm_id,
                uri: uri.into(),
                line,
                column,
                verified,
                condition: conds.condition,
                hit_condition: conds.hit_condition,
                hit_count: 0,
                log_message: conds.log_message,
            },
        );
        dap_id
    }

    /// Increment the hit count for a breakpoint by VM ID and return the new count.
    ///
    /// Returns `None` if no breakpoint with the given VM ID is tracked.
    pub fn increment_hit_count(&mut self, vm_id: &str) -> Option<u64> {
        let dap_id = *self.vm_id_to_dap_id.get(vm_id)?;
        let entry = self.by_dap_id.get_mut(&dap_id)?;
        entry.hit_count += 1;
        Some(entry.hit_count)
    }

    /// Remove a breakpoint by its DAP ID.
    ///
    /// Returns the removed entry, or `None` if no breakpoint with that ID exists.
    pub fn remove_by_dap_id(&mut self, dap_id: i64) -> Option<BreakpointEntry> {
        if let Some(entry) = self.by_dap_id.remove(&dap_id) {
            self.vm_id_to_dap_id.remove(&entry.vm_id);
            Some(entry)
        } else {
            None
        }
    }

    /// Remove all breakpoints for a given source URI.
    ///
    /// Returns the list of removed entries (in unspecified order). The caller
    /// should remove each from the VM Service using the `vm_id` field.
    pub fn remove_all_for_uri(&mut self, uri: &str) -> Vec<BreakpointEntry> {
        let to_remove: Vec<i64> = self
            .by_dap_id
            .values()
            .filter(|e| e.uri == uri)
            .map(|e| e.dap_id)
            .collect();

        to_remove
            .into_iter()
            .filter_map(|id| self.remove_by_dap_id(id))
            .collect()
    }

    /// Look up a breakpoint by its DAP ID.
    pub fn lookup_by_dap_id(&self, dap_id: i64) -> Option<&BreakpointEntry> {
        self.by_dap_id.get(&dap_id)
    }

    /// Look up a breakpoint by its VM Service ID.
    pub fn lookup_by_vm_id(&self, vm_id: &str) -> Option<&BreakpointEntry> {
        let dap_id = self.vm_id_to_dap_id.get(vm_id)?;
        self.by_dap_id.get(dap_id)
    }

    /// Mark a breakpoint as resolved and update its location.
    ///
    /// Called when a `BreakpointResolved` VM Service event arrives. Returns
    /// a reference to the updated entry, or `None` if the VM ID is not tracked.
    pub fn resolve_breakpoint(
        &mut self,
        vm_id: &str,
        line: Option<i32>,
        column: Option<i32>,
    ) -> Option<&BreakpointEntry> {
        let dap_id = *self.vm_id_to_dap_id.get(vm_id)?;
        if let Some(entry) = self.by_dap_id.get_mut(&dap_id) {
            entry.verified = true;
            if line.is_some() {
                entry.line = line;
            }
            if column.is_some() {
                entry.column = column;
            }
        }
        self.by_dap_id.get(&dap_id)
    }

    /// Return the total number of tracked breakpoints.
    pub fn len(&self) -> usize {
        self.by_dap_id.len()
    }

    /// Return `true` if no breakpoints are tracked.
    pub fn is_empty(&self) -> bool {
        self.by_dap_id.is_empty()
    }

    /// Iterate over all breakpoints in unspecified order.
    pub fn iter(&self) -> impl Iterator<Item = &BreakpointEntry> {
        self.by_dap_id.values()
    }

    /// Find breakpoints for a source URI, returning all entries.
    ///
    /// Returns an iterator over breakpoints that match the given URI.
    pub fn iter_for_uri<'a>(&'a self, uri: &'a str) -> impl Iterator<Item = &'a BreakpointEntry> {
        self.by_dap_id.values().filter(move |e| e.uri == uri)
    }

    /// Find the DAP ID for an existing breakpoint at the given URI and line.
    ///
    /// Returns `None` if no breakpoint exists at that location.
    pub fn find_by_source_line(&self, uri: &str, line: i64) -> Option<i64> {
        self.by_dap_id
            .values()
            .find(|e| e.uri == uri && e.line == Some(line as i32))
            .map(|e| e.dap_id)
    }

    /// Remove and return all tracked breakpoints.
    ///
    /// Used by [`BreakpointManager::clear_active`] on isolate exit.  The
    /// returned entries carry the DAP IDs and VM IDs of the cleared
    /// breakpoints so callers can emit `breakpoint changed` events.
    pub fn drain_all(&mut self) -> Vec<BreakpointEntry> {
        let entries: Vec<BreakpointEntry> = self.by_dap_id.drain().map(|(_, v)| v).collect();
        self.vm_id_to_dap_id.clear();
        entries
    }

    /// Insert a breakpoint with a **pre-assigned** DAP ID.
    ///
    /// Unlike [`add`] and [`add_with_condition`], this method does not allocate
    /// a new DAP ID — it reuses the provided one. The `next_dap_id` counter
    /// is not updated by this method.
    ///
    /// Used by [`BreakpointManager::record_active`] when re-registering
    /// breakpoints after a hot restart using their stable desired-state IDs.
    #[allow(clippy::too_many_arguments)]
    pub fn insert_with_id(
        &mut self,
        dap_id: i64,
        vm_id: impl Into<String>,
        uri: impl Into<String>,
        line: Option<i32>,
        column: Option<i32>,
        verified: bool,
        conds: BreakpointCondition,
    ) {
        let vm_id = vm_id.into();
        self.vm_id_to_dap_id.insert(vm_id.clone(), dap_id);
        self.by_dap_id.insert(
            dap_id,
            BreakpointEntry {
                dap_id,
                vm_id,
                uri: uri.into(),
                line,
                column,
                verified,
                condition: conds.condition,
                hit_condition: conds.hit_condition,
                hit_count: 0,
                log_message: conds.log_message,
            },
        );
    }
}

impl Default for BreakpointState {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// DesiredBreakpoint
// ─────────────────────────────────────────────────────────────────────────────

/// A breakpoint as requested by the IDE, independent of VM state.
///
/// Survives hot restart. The DAP ID is stable across restarts so the IDE
/// can correlate `breakpoint changed` events with previously reported
/// breakpoints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesiredBreakpoint {
    /// DAP breakpoint ID — stable across restarts.
    pub dap_id: i64,
    /// Requested source line (1-based).
    pub line: i32,
    /// Requested source column (1-based), if specified.
    pub column: Option<i32>,
    /// An optional Dart expression condition.
    pub condition: Option<String>,
    /// An optional hit-count expression (e.g., `">= 3"`).
    pub hit_condition: Option<String>,
    /// An optional logpoint message template.
    pub log_message: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// BreakpointManager
// ─────────────────────────────────────────────────────────────────────────────

/// A desired breakpoint descriptor passed to [`BreakpointManager::set_desired`].
///
/// `(line, column, condition, hit_condition, log_message)`
pub type DesiredBreakpointSpec = (
    i32,
    Option<i32>,
    Option<String>,
    Option<String>,
    Option<String>,
);

/// Manages both desired and active breakpoint state across hot restarts.
///
/// The adapter holds a single `BreakpointManager` which keeps:
///
/// - **desired** — what the IDE asked for, keyed by source URI.  Survives hot
///   restart.  DAP IDs here are monotonically assigned and stable.
/// - **active** — what is currently set in the VM (`BreakpointState`).
///   Cleared on isolate exit; rebuilt on `IsolateRunnable`.
///
/// # Hot restart flow
///
/// 1. `IsolateExit` → call [`BreakpointManager::clear_active`].  The desired
///    set is untouched.  Callers emit `breakpoint changed` with `verified: false`
///    for every desired breakpoint using [`BreakpointManager::desired_iter`].
/// 2. `IsolateRunnable` → iterate [`BreakpointManager::desired_by_uri`] and
///    call `addBreakpointWithScriptUri` for each.  Then call
///    [`BreakpointManager::record_active`] with the returned VM ID, and emit
///    `breakpoint changed` with `verified: true`.
pub struct BreakpointManager {
    /// What the IDE wants, keyed by source URI.
    desired: HashMap<String, Vec<DesiredBreakpoint>>,

    /// What is currently set in the VM.
    active: BreakpointState,

    /// Next DAP ID to allocate. Monotonically increasing, never reset.
    next_dap_id: i64,
}

impl BreakpointManager {
    /// Create a new, empty [`BreakpointManager`].
    pub fn new() -> Self {
        Self {
            desired: HashMap::new(),
            active: BreakpointState::new(),
            next_dap_id: 1,
        }
    }

    // ── Desired-state management ──────────────────────────────────────────

    /// Replace the complete set of desired breakpoints for `uri`.
    ///
    /// Returns the newly allocated desired breakpoints so callers can
    /// immediately attempt to add them to the VM.
    ///
    /// Previous desired breakpoints for the same URI are discarded.  The DAP
    /// IDs in the returned slice are freshly allocated and stable across
    /// restarts.
    pub fn set_desired(
        &mut self,
        uri: &str,
        lines: &[DesiredBreakpointSpec],
    ) -> Vec<DesiredBreakpoint> {
        let mut result = Vec::with_capacity(lines.len());
        for (line, column, condition, hit_condition, log_message) in lines {
            let dap_id = self.next_dap_id;
            self.next_dap_id += 1;
            result.push(DesiredBreakpoint {
                dap_id,
                line: *line,
                column: *column,
                condition: condition.clone(),
                hit_condition: hit_condition.clone(),
                log_message: log_message.clone(),
            });
        }
        self.desired.insert(uri.to_string(), result.clone());
        result
    }

    /// Return desired breakpoints for a specific URI.
    pub fn desired_for(&self, uri: &str) -> &[DesiredBreakpoint] {
        self.desired.get(uri).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Iterate over all desired breakpoints across all URIs.
    ///
    /// Yields `(uri, desired_breakpoint)` pairs.
    pub fn desired_iter(&self) -> impl Iterator<Item = (&str, &DesiredBreakpoint)> {
        self.desired
            .iter()
            .flat_map(|(uri, bps)| bps.iter().map(move |bp| (uri.as_str(), bp)))
    }

    /// Return all source URIs that have at least one desired breakpoint.
    pub fn desired_uris(&self) -> impl Iterator<Item = &str> {
        self.desired.keys().map(String::as_str)
    }

    /// Remove the desired breakpoints for a URI, returning the removed entries.
    pub fn clear_desired_for_uri(&mut self, uri: &str) -> Vec<DesiredBreakpoint> {
        self.desired.remove(uri).unwrap_or_default()
    }

    // ── Active-state management ───────────────────────────────────────────

    /// Register a newly added VM breakpoint, linking it to a desired DAP ID.
    ///
    /// `dap_id` must match a previously allocated ID from [`set_desired`].
    /// `vm_id` is the opaque string returned by the VM Service.
    #[allow(clippy::too_many_arguments)]
    pub fn record_active(
        &mut self,
        dap_id: i64,
        vm_id: impl Into<String>,
        uri: impl Into<String>,
        line: Option<i32>,
        column: Option<i32>,
        verified: bool,
        conds: BreakpointCondition,
    ) {
        let vm_id = vm_id.into();
        // Insert directly with the given dap_id instead of auto-allocating.
        self.active
            .insert_with_id(dap_id, vm_id, uri, line, column, verified, conds);
    }

    /// Clear all active (VM-tracked) breakpoints.
    ///
    /// Called on `IsolateExit`. The desired set is not modified.
    /// Returns the list of removed entries so callers can emit unverified events.
    pub fn clear_active(&mut self) -> Vec<BreakpointEntry> {
        self.active.drain_all()
    }

    /// Access the active breakpoint state for read operations.
    pub fn active(&self) -> &BreakpointState {
        &self.active
    }

    /// Access the active breakpoint state for mutation (e.g., hit count, resolve).
    pub fn active_mut(&mut self) -> &mut BreakpointState {
        &mut self.active
    }

    /// Find an active breakpoint by VM ID.
    pub fn lookup_active_by_vm_id(&self, vm_id: &str) -> Option<&BreakpointEntry> {
        self.active.lookup_by_vm_id(vm_id)
    }

    /// Find an active breakpoint by DAP ID.
    pub fn lookup_active_by_dap_id(&self, dap_id: i64) -> Option<&BreakpointEntry> {
        self.active.lookup_by_dap_id(dap_id)
    }

    /// Resolve an active breakpoint (update verified/line/column).
    pub fn resolve_active(
        &mut self,
        vm_id: &str,
        line: Option<i32>,
        column: Option<i32>,
    ) -> Option<&BreakpointEntry> {
        self.active.resolve_breakpoint(vm_id, line, column)
    }

    /// Remove an active breakpoint by its VM ID.
    ///
    /// Returns the removed entry so the caller can tell the VM Service to
    /// delete the breakpoint.
    pub fn remove_active_by_vm_id(&mut self, vm_id: &str) -> Option<BreakpointEntry> {
        // Locate the DAP ID first, then remove by DAP ID.
        let dap_id = *self.active.vm_id_to_dap_id.get(vm_id)?;
        self.active.remove_by_dap_id(dap_id)
    }

    /// Peek at the next DAP ID that will be allocated (for testing).
    #[cfg(test)]
    pub fn next_dap_id(&self) -> i64 {
        self.next_dap_id
    }
}

impl Default for BreakpointManager {
    fn default() -> Self {
        Self::new()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Hit condition evaluation
// ─────────────────────────────────────────────────────────────────────────────

/// Evaluate a hit-condition string against the current hit count.
///
/// The hit condition is a simple arithmetic expression comparing the hit count
/// to a literal integer. Supported formats:
///
/// | Format         | Meaning                               |
/// |----------------|---------------------------------------|
/// | `"== N"`       | Fire only on the Nth hit              |
/// | `">= N"`       | Fire on the Nth and all subsequent hits |
/// | `"<= N"`       | Fire on hits 1 through N              |
/// | `"> N"`        | Fire on hits strictly after N         |
/// | `"< N"`        | Fire on hits strictly before N        |
/// | `"% N"`        | Fire every Nth hit (modulo — returns true when `hit_count % N == 0`) |
/// | `"% N == 0"`   | Same as `"% N"` (explicit form)       |
///
/// Returns `true` if the condition passes (the debugger should pause),
/// `false` if the condition does not pass (the debugger should silently
/// resume). Returns `true` when the condition string cannot be parsed
/// (safe default: stop rather than silently miss the breakpoint).
///
/// # Examples
///
/// ```rust
/// # use fdemon_dap::adapter::breakpoints::evaluate_hit_condition;
/// assert!(evaluate_hit_condition(3, ">= 3"));
/// assert!(!evaluate_hit_condition(2, ">= 3"));
/// assert!(evaluate_hit_condition(5, "== 5"));
/// assert!(evaluate_hit_condition(4, "% 2 == 0"));
/// assert!(!evaluate_hit_condition(3, "% 2 == 0"));
/// ```
pub fn evaluate_hit_condition(hit_count: u64, condition: &str) -> bool {
    let condition = condition.trim();

    // Handle modulo operator: "% N" or "% N == 0"
    if let Some(rest) = condition.strip_prefix('%') {
        // Trim the rest and extract the modulus value (ignore trailing "== 0").
        let rest = rest.trim();
        // Accept "% N" or "% N == 0"
        let modulus_str = rest.split_whitespace().next().unwrap_or("");
        return match modulus_str.parse::<u64>() {
            Ok(n) if n > 0 => hit_count.is_multiple_of(n),
            // Parse failure or zero divisor → safe default: stop
            _ => true,
        };
    }

    // Handle comparison operators: "== N", ">= N", "<= N", "> N", "< N"
    let (op, rhs_str) = if let Some(rest) = condition.strip_prefix("==") {
        ("==", rest.trim())
    } else if let Some(rest) = condition.strip_prefix(">=") {
        (">=", rest.trim())
    } else if let Some(rest) = condition.strip_prefix("<=") {
        ("<=", rest.trim())
    } else if let Some(rest) = condition.strip_prefix('>') {
        (">", rest.trim())
    } else if let Some(rest) = condition.strip_prefix('<') {
        ("<", rest.trim())
    } else {
        // Unrecognised format — safe default: stop
        return true;
    };

    // Only the first token is the RHS integer (ignore trailing text).
    let rhs_str = rhs_str.split_whitespace().next().unwrap_or("");
    let rhs: u64 = match rhs_str.parse() {
        Ok(n) => n,
        // Parse failure → safe default: stop
        Err(_) => return true,
    };

    match op {
        "==" => hit_count == rhs,
        ">=" => hit_count >= rhs,
        "<=" => hit_count <= rhs,
        ">" => hit_count > rhs,
        "<" => hit_count < rhs,
        // Should not be reached, but safe default: stop
        _ => true,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Dart value truthiness
// ─────────────────────────────────────────────────────────────────────────────

/// Determine if a Dart VM Service `InstanceRef`/`Instance` JSON value is truthy.
///
/// Dart's truthiness rules are much simpler than JavaScript's:
///
/// | VM `kind`    | `valueAsString` | Truthy? |
/// |--------------|-----------------|---------|
/// | `"Bool"`     | `"true"`        | yes     |
/// | `"Bool"`     | `"false"`       | no      |
/// | `"Null"`     | (any)           | no      |
/// | anything else| (any)           | yes     |
///
/// This matches the evaluation semantics expected by conditional breakpoints:
/// a `condition` expression should be a boolean expression in Dart code.
/// Non-bool, non-null results (e.g., an integer) are treated as truthy so
/// that the debugger stops — the developer likely made a mistake in the
/// condition expression and a stop is safer than a silent skip.
pub fn is_truthy(result: &serde_json::Value) -> bool {
    let kind = result.get("kind").and_then(|k| k.as_str());
    match kind {
        Some("Bool") => result.get("valueAsString").and_then(|v| v.as_str()) == Some("true"),
        Some("Null") => false,
        // Non-null, non-bool values are truthy.
        _ => true,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_log_message ─────────────────────────────────────────────────

    #[test]
    fn test_parse_log_message_no_expressions_returns_single_literal() {
        let segments = parse_log_message("Hello world");
        assert_eq!(segments.len(), 1);
        assert!(
            matches!(&segments[0], LogSegment::Literal(s) if s == "Hello world"),
            "Expected single literal segment"
        );
    }

    #[test]
    fn test_parse_log_message_empty_string_returns_empty() {
        let segments = parse_log_message("");
        assert!(
            segments.is_empty(),
            "Empty template should produce no segments"
        );
    }

    #[test]
    fn test_parse_log_message_single_expression_at_end() {
        let segments = parse_log_message("x = {x}");
        assert_eq!(segments.len(), 2);
        assert!(matches!(&segments[0], LogSegment::Literal(s) if s == "x = "));
        assert!(matches!(&segments[1], LogSegment::Expression(s) if s == "x"));
    }

    #[test]
    fn test_parse_log_message_expression_only() {
        let segments = parse_log_message("{myVar}");
        assert_eq!(segments.len(), 1);
        assert!(matches!(&segments[0], LogSegment::Expression(s) if s == "myVar"));
    }

    #[test]
    fn test_parse_log_message_multiple_expressions() {
        let segments = parse_log_message("({a}, {b})");
        // Expected: Literal("("), Expression("a"), Literal(", "), Expression("b"), Literal(")")
        assert_eq!(segments.len(), 5, "Expected 5 segments: got {:?}", segments);
        assert!(matches!(&segments[0], LogSegment::Literal(s) if s == "("));
        assert!(matches!(&segments[1], LogSegment::Expression(s) if s == "a"));
        assert!(matches!(&segments[2], LogSegment::Literal(s) if s == ", "));
        assert!(matches!(&segments[3], LogSegment::Expression(s) if s == "b"));
        assert!(matches!(&segments[4], LogSegment::Literal(s) if s == ")"));
    }

    #[test]
    fn test_parse_log_message_expression_at_start() {
        let segments = parse_log_message("{x} is the value");
        assert_eq!(segments.len(), 2);
        assert!(matches!(&segments[0], LogSegment::Expression(s) if s == "x"));
        assert!(matches!(&segments[1], LogSegment::Literal(s) if s == " is the value"));
    }

    #[test]
    fn test_parse_log_message_unmatched_brace_treated_as_literal() {
        // Unmatched `{` — rest of the string becomes a literal.
        let segments = parse_log_message("unclosed {brace");
        // "unclosed " is consumed up to '{', then the unmatched brace causes
        // the whole remaining string from '{' onward to be a literal.
        // Result: [Literal("unclosed "), Literal("{brace")]
        let joined: String = segments
            .iter()
            .map(|s| match s {
                LogSegment::Literal(t) => t.as_str(),
                LogSegment::Expression(t) => t.as_str(),
            })
            .collect();
        assert_eq!(joined, "unclosed {brace", "All text should be preserved");
        // All segments should be Literal (no Expression for unmatched brace).
        for seg in &segments {
            assert!(
                matches!(seg, LogSegment::Literal(_)),
                "Unmatched brace should not produce Expression segments, got: {:?}",
                seg
            );
        }
    }

    #[test]
    fn test_parse_log_message_empty_expression() {
        // {} is an empty expression — still parsed as an Expression("").
        let segments = parse_log_message("before {} after");
        assert_eq!(segments.len(), 3);
        assert!(matches!(&segments[0], LogSegment::Literal(s) if s == "before "));
        assert!(matches!(&segments[1], LogSegment::Expression(s) if s.is_empty()));
        assert!(matches!(&segments[2], LogSegment::Literal(s) if s == " after"));
    }

    #[test]
    fn test_parse_log_message_adjacent_expressions() {
        let segments = parse_log_message("{a}{b}");
        assert_eq!(segments.len(), 2);
        assert!(matches!(&segments[0], LogSegment::Expression(s) if s == "a"));
        assert!(matches!(&segments[1], LogSegment::Expression(s) if s == "b"));
    }

    #[test]
    fn test_parse_log_message_nested_property_access() {
        let segments = parse_log_message("Point: ({point.x}, {point.y})");
        // Expected: Literal("Point: ("), Expr("point.x"), Literal(", "), Expr("point.y"), Literal(")")
        assert_eq!(segments.len(), 5);
        assert!(matches!(&segments[1], LogSegment::Expression(s) if s == "point.x"));
        assert!(matches!(&segments[3], LogSegment::Expression(s) if s == "point.y"));
    }

    // ── BreakpointCondition log_message field ─────────────────────────────

    #[test]
    fn test_breakpoint_condition_default_has_no_log_message() {
        let cond = BreakpointCondition::default();
        assert_eq!(cond.log_message, None);
    }

    // ── BreakpointEntry log_message field ─────────────────────────────────

    #[test]
    fn test_add_with_condition_stores_log_message() {
        let mut state = BreakpointState::new();
        let id = state.add_with_condition(
            "bp/1",
            "file:///lib/main.dart",
            Some(42),
            None,
            true,
            BreakpointCondition {
                condition: None,
                hit_condition: None,
                log_message: Some("x = {x}".to_string()),
            },
        );
        let entry = state.lookup_by_dap_id(id).expect("Entry must exist");
        assert_eq!(entry.log_message.as_deref(), Some("x = {x}"));
    }

    #[test]
    fn test_add_unconditional_has_no_log_message() {
        let mut state = BreakpointState::new();
        let id = state.add("bp/1", "file:///lib/main.dart", Some(10), None, true);
        let entry = state.lookup_by_dap_id(id).expect("Entry must exist");
        assert_eq!(entry.log_message, None);
    }

    // ── existing tests below (unchanged) ──────────────────────────────────

    #[test]
    fn test_breakpoint_state_starts_empty() {
        let state = BreakpointState::new();
        assert!(state.is_empty());
        assert_eq!(state.len(), 0);
    }

    #[test]
    fn test_breakpoint_state_add_returns_monotonic_dap_ids() {
        let mut state = BreakpointState::new();
        let id1 = state.add("bp/1", "file:///lib/main.dart", Some(10), None, false);
        let id2 = state.add("bp/2", "file:///lib/main.dart", Some(20), None, false);
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
    }

    #[test]
    fn test_breakpoint_state_add_first_id_is_one() {
        let mut state = BreakpointState::new();
        let id = state.add("bp/1", "file:///lib/main.dart", Some(5), None, false);
        assert_eq!(id, 1, "First DAP breakpoint ID must be 1");
    }

    #[test]
    fn test_breakpoint_state_lookup_by_dap_id() {
        let mut state = BreakpointState::new();
        let id = state.add("bp/42", "file:///lib/foo.dart", Some(15), Some(3), true);
        let entry = state.lookup_by_dap_id(id).expect("Entry should exist");
        assert_eq!(entry.dap_id, id);
        assert_eq!(entry.vm_id, "bp/42");
        assert_eq!(entry.uri, "file:///lib/foo.dart");
        assert_eq!(entry.line, Some(15));
        assert_eq!(entry.column, Some(3));
        assert!(entry.verified);
    }

    #[test]
    fn test_breakpoint_state_lookup_by_dap_id_returns_none_for_unknown() {
        let state = BreakpointState::new();
        assert!(state.lookup_by_dap_id(99).is_none());
    }

    #[test]
    fn test_breakpoint_state_lookup_by_vm_id() {
        let mut state = BreakpointState::new();
        let id = state.add("bp/99", "file:///lib/bar.dart", Some(7), None, false);
        let entry = state.lookup_by_vm_id("bp/99").expect("Entry should exist");
        assert_eq!(entry.dap_id, id);
    }

    #[test]
    fn test_breakpoint_state_lookup_by_vm_id_returns_none_for_unknown() {
        let state = BreakpointState::new();
        assert!(state.lookup_by_vm_id("bp/unknown").is_none());
    }

    #[test]
    fn test_breakpoint_state_remove_by_dap_id() {
        let mut state = BreakpointState::new();
        let id = state.add("bp/1", "file:///lib/main.dart", Some(10), None, false);
        let removed = state
            .remove_by_dap_id(id)
            .expect("Should remove existing entry");
        assert_eq!(removed.dap_id, id);
        assert!(state.lookup_by_dap_id(id).is_none());
        assert!(state.lookup_by_vm_id("bp/1").is_none());
        assert!(state.is_empty());
    }

    #[test]
    fn test_breakpoint_state_remove_by_dap_id_returns_none_for_unknown() {
        let mut state = BreakpointState::new();
        assert!(state.remove_by_dap_id(99).is_none());
    }

    #[test]
    fn test_breakpoint_state_remove_all_for_uri() {
        let mut state = BreakpointState::new();
        state.add("bp/1", "file:///lib/main.dart", Some(10), None, false);
        state.add("bp/2", "file:///lib/main.dart", Some(20), None, false);
        state.add("bp/3", "file:///lib/other.dart", Some(5), None, false);

        let removed = state.remove_all_for_uri("file:///lib/main.dart");
        assert_eq!(
            removed.len(),
            2,
            "Should remove 2 breakpoints from main.dart"
        );
        assert_eq!(state.len(), 1, "other.dart breakpoint should remain");
        // The remaining breakpoint is in other.dart.
        assert!(state.lookup_by_vm_id("bp/3").is_some());
    }

    #[test]
    fn test_breakpoint_state_remove_all_for_uri_unknown_returns_empty() {
        let mut state = BreakpointState::new();
        state.add("bp/1", "file:///lib/main.dart", Some(10), None, false);
        let removed = state.remove_all_for_uri("file:///lib/does_not_exist.dart");
        assert!(removed.is_empty());
        assert_eq!(state.len(), 1);
    }

    #[test]
    fn test_breakpoint_state_resolve_breakpoint() {
        let mut state = BreakpointState::new();
        state.add("bp/1", "file:///lib/main.dart", Some(10), None, false);

        let entry = state
            .resolve_breakpoint("bp/1", Some(11), Some(3))
            .expect("Should resolve known breakpoint");
        assert!(entry.verified);
        assert_eq!(entry.line, Some(11));
        assert_eq!(entry.column, Some(3));
    }

    #[test]
    fn test_breakpoint_state_resolve_preserves_existing_line_when_none_passed() {
        let mut state = BreakpointState::new();
        state.add("bp/1", "file:///lib/main.dart", Some(10), None, false);
        // Resolve with no new line — should keep existing line.
        state.resolve_breakpoint("bp/1", None, None);
        let entry = state.lookup_by_vm_id("bp/1").unwrap();
        assert_eq!(
            entry.line,
            Some(10),
            "Line should be preserved when None is passed"
        );
    }

    #[test]
    fn test_breakpoint_state_resolve_unknown_vm_id_returns_none() {
        let mut state = BreakpointState::new();
        assert!(state
            .resolve_breakpoint("bp/unknown", Some(5), None)
            .is_none());
    }

    #[test]
    fn test_breakpoint_state_len_tracks_additions_and_removals() {
        let mut state = BreakpointState::new();
        assert_eq!(state.len(), 0);
        let id = state.add("bp/1", "file:///lib/main.dart", Some(1), None, false);
        assert_eq!(state.len(), 1);
        state.add("bp/2", "file:///lib/main.dart", Some(2), None, false);
        assert_eq!(state.len(), 2);
        state.remove_by_dap_id(id);
        assert_eq!(state.len(), 1);
    }

    #[test]
    fn test_breakpoint_state_iter_returns_all_entries() {
        let mut state = BreakpointState::new();
        state.add("bp/1", "file:///lib/a.dart", Some(1), None, false);
        state.add("bp/2", "file:///lib/b.dart", Some(2), None, false);
        state.add("bp/3", "file:///lib/c.dart", Some(3), None, false);

        let count = state.iter().count();
        assert_eq!(count, 3);
    }

    // ── add_with_condition ────────────────────────────────────────────────

    #[test]
    fn test_add_with_condition_stores_condition() {
        let mut state = BreakpointState::new();
        let id = state.add_with_condition(
            "bp/1",
            "file:///lib/main.dart",
            Some(10),
            None,
            false,
            BreakpointCondition {
                condition: Some("x > 5".to_string()),
                hit_condition: None,
                log_message: None,
            },
        );
        let entry = state.lookup_by_dap_id(id).expect("Entry must exist");
        assert_eq!(entry.condition.as_deref(), Some("x > 5"));
        assert_eq!(entry.hit_condition, None);
        assert_eq!(entry.hit_count, 0);
    }

    #[test]
    fn test_add_with_condition_stores_hit_condition() {
        let mut state = BreakpointState::new();
        let id = state.add_with_condition(
            "bp/2",
            "file:///lib/main.dart",
            Some(20),
            None,
            false,
            BreakpointCondition {
                condition: None,
                hit_condition: Some(">= 3".to_string()),
                log_message: None,
            },
        );
        let entry = state.lookup_by_dap_id(id).expect("Entry must exist");
        assert_eq!(entry.condition, None);
        assert_eq!(entry.hit_condition.as_deref(), Some(">= 3"));
    }

    #[test]
    fn test_add_unconditional_has_none_fields() {
        let mut state = BreakpointState::new();
        let id = state.add("bp/1", "file:///lib/main.dart", Some(10), None, true);
        let entry = state.lookup_by_dap_id(id).expect("Entry must exist");
        assert_eq!(entry.condition, None);
        assert_eq!(entry.hit_condition, None);
        assert_eq!(entry.hit_count, 0);
    }

    // ── increment_hit_count ───────────────────────────────────────────────

    #[test]
    fn test_increment_hit_count_starts_at_zero_then_increments() {
        let mut state = BreakpointState::new();
        state.add("bp/1", "file:///lib/main.dart", Some(10), None, true);

        assert_eq!(state.increment_hit_count("bp/1"), Some(1));
        assert_eq!(state.increment_hit_count("bp/1"), Some(2));
        assert_eq!(state.increment_hit_count("bp/1"), Some(3));
    }

    #[test]
    fn test_increment_hit_count_unknown_vm_id_returns_none() {
        let mut state = BreakpointState::new();
        assert_eq!(state.increment_hit_count("bp/unknown"), None);
    }

    // ── evaluate_hit_condition ────────────────────────────────────────────

    #[test]
    fn test_hit_condition_eq_exact_match() {
        assert!(!evaluate_hit_condition(4, "== 5"));
        assert!(evaluate_hit_condition(5, "== 5"));
        assert!(!evaluate_hit_condition(6, "== 5"));
    }

    #[test]
    fn test_hit_condition_gte_fires_on_threshold_and_above() {
        assert!(!evaluate_hit_condition(1, ">= 3"));
        assert!(!evaluate_hit_condition(2, ">= 3"));
        assert!(evaluate_hit_condition(3, ">= 3"));
        assert!(evaluate_hit_condition(4, ">= 3"));
        assert!(evaluate_hit_condition(100, ">= 3"));
    }

    #[test]
    fn test_hit_condition_lte_fires_up_to_threshold() {
        assert!(evaluate_hit_condition(1, "<= 3"));
        assert!(evaluate_hit_condition(3, "<= 3"));
        assert!(!evaluate_hit_condition(4, "<= 3"));
    }

    #[test]
    fn test_hit_condition_gt_fires_strictly_above() {
        assert!(!evaluate_hit_condition(3, "> 3"));
        assert!(evaluate_hit_condition(4, "> 3"));
    }

    #[test]
    fn test_hit_condition_lt_fires_strictly_below() {
        assert!(evaluate_hit_condition(2, "< 3"));
        assert!(!evaluate_hit_condition(3, "< 3"));
        assert!(!evaluate_hit_condition(4, "< 3"));
    }

    #[test]
    fn test_hit_condition_modulo_fires_on_multiples() {
        // Every 2nd hit: 2, 4, 6, ...
        assert!(!evaluate_hit_condition(1, "% 2 == 0"));
        assert!(evaluate_hit_condition(2, "% 2 == 0"));
        assert!(!evaluate_hit_condition(3, "% 2 == 0"));
        assert!(evaluate_hit_condition(4, "% 2 == 0"));
    }

    #[test]
    fn test_hit_condition_modulo_short_form() {
        // "% N" without "== 0" is also valid
        assert!(evaluate_hit_condition(2, "% 2"));
        assert!(!evaluate_hit_condition(3, "% 2"));
        assert!(evaluate_hit_condition(6, "% 3"));
    }

    #[test]
    fn test_hit_condition_invalid_format_returns_true_safe_default() {
        // Unparseable conditions → stop (safe default)
        assert!(evaluate_hit_condition(1, "not_a_number"));
        assert!(evaluate_hit_condition(5, ""));
        assert!(evaluate_hit_condition(5, "?? 3"));
    }

    #[test]
    fn test_hit_condition_modulo_zero_divisor_returns_true_safe_default() {
        // Division by zero → stop (safe default)
        assert!(evaluate_hit_condition(5, "% 0"));
    }

    #[test]
    fn test_hit_condition_whitespace_trimmed() {
        assert!(evaluate_hit_condition(5, "  == 5  "));
        assert!(evaluate_hit_condition(3, "  >= 3  "));
    }

    // ── is_truthy ─────────────────────────────────────────────────────────

    #[test]
    fn test_is_truthy_bool_true() {
        let val = serde_json::json!({"kind": "Bool", "valueAsString": "true"});
        assert!(is_truthy(&val));
    }

    #[test]
    fn test_is_truthy_bool_false() {
        let val = serde_json::json!({"kind": "Bool", "valueAsString": "false"});
        assert!(!is_truthy(&val));
    }

    #[test]
    fn test_is_truthy_null_is_falsy() {
        let val = serde_json::json!({"kind": "Null"});
        assert!(!is_truthy(&val));
    }

    #[test]
    fn test_is_truthy_int_is_truthy() {
        let val = serde_json::json!({"kind": "Int", "valueAsString": "42"});
        assert!(is_truthy(&val));
    }

    #[test]
    fn test_is_truthy_string_is_truthy() {
        let val = serde_json::json!({"kind": "String", "valueAsString": "hello"});
        assert!(is_truthy(&val));
    }

    #[test]
    fn test_is_truthy_plain_instance_is_truthy() {
        let val = serde_json::json!({
            "kind": "PlainInstance",
            "class": {"name": "MyClass"}
        });
        assert!(is_truthy(&val));
    }

    #[test]
    fn test_is_truthy_empty_object_is_truthy() {
        // Missing "kind" field → treated as unknown → truthy (safe default: stop)
        let val = serde_json::json!({});
        assert!(is_truthy(&val));
    }

    // ── BreakpointState::drain_all ────────────────────────────────────────

    #[test]
    fn test_drain_all_removes_all_entries_and_returns_them() {
        let mut state = BreakpointState::new();
        state.add("bp/1", "file:///lib/main.dart", Some(10), None, true);
        state.add("bp/2", "file:///lib/main.dart", Some(20), None, true);
        state.add("bp/3", "file:///lib/other.dart", Some(5), None, false);

        let drained = state.drain_all();
        assert_eq!(drained.len(), 3, "Should drain all 3 entries");
        assert!(state.is_empty(), "State must be empty after drain_all");
        // vm_id_to_dap_id index must also be cleared.
        assert!(state.lookup_by_vm_id("bp/1").is_none());
        assert!(state.lookup_by_vm_id("bp/2").is_none());
        assert!(state.lookup_by_vm_id("bp/3").is_none());
    }

    #[test]
    fn test_drain_all_on_empty_state_returns_empty_vec() {
        let mut state = BreakpointState::new();
        let drained = state.drain_all();
        assert!(
            drained.is_empty(),
            "drain_all on empty state should return empty vec"
        );
    }

    // ── BreakpointState::insert_with_id ──────────────────────────────────

    #[test]
    fn test_insert_with_id_uses_provided_dap_id() {
        let mut state = BreakpointState::new();
        state.insert_with_id(
            42,
            "bp/vm-1",
            "file:///lib/main.dart",
            Some(10),
            None,
            true,
            BreakpointCondition::default(),
        );
        let entry = state
            .lookup_by_dap_id(42)
            .expect("Entry should exist with id 42");
        assert_eq!(entry.dap_id, 42);
        assert_eq!(entry.vm_id, "bp/vm-1");
        assert_eq!(entry.uri, "file:///lib/main.dart");
        assert_eq!(entry.line, Some(10));
        assert!(entry.verified);
    }

    #[test]
    fn test_insert_with_id_does_not_advance_next_dap_id_counter() {
        // The auto-allocating add() should still start from 1 after insert_with_id.
        let mut state = BreakpointState::new();
        state.insert_with_id(
            100,
            "bp/vm-100",
            "file:///lib/main.dart",
            Some(10),
            None,
            true,
            BreakpointCondition::default(),
        );
        // insert_with_id should not affect next_dap_id.
        let auto_id = state.add("bp/vm-auto", "file:///lib/other.dart", Some(5), None, false);
        assert_eq!(auto_id, 1, "Auto-allocated ID should start from 1");
    }

    // ── BreakpointManager ─────────────────────────────────────────────────

    /// Helper: build a simple desired breakpoint descriptor tuple.
    fn bp_line(line: i32) -> super::DesiredBreakpointSpec {
        (line, None, None, None, None)
    }

    /// Helper: build a desired breakpoint with a condition.
    fn bp_with_cond(line: i32, condition: &str) -> super::DesiredBreakpointSpec {
        (line, None, Some(condition.to_string()), None, None)
    }

    /// Helper: build a desired breakpoint with a logpoint message.
    fn bp_with_log(line: i32, log: &str) -> super::DesiredBreakpointSpec {
        (line, None, None, None, Some(log.to_string()))
    }

    #[test]
    fn test_manager_desired_breakpoints_survive_clear_active() {
        let mut mgr = BreakpointManager::new();
        mgr.set_desired("file:///main.dart", &[bp_line(25), bp_line(30)]);

        // Simulate hot restart: clear active state.
        mgr.clear_active();

        // Desired state must still be intact.
        let desired = mgr.desired_for("file:///main.dart");
        assert_eq!(
            desired.len(),
            2,
            "Desired breakpoints must survive clear_active"
        );
        assert_eq!(desired[0].line, 25);
        assert_eq!(desired[1].line, 30);
    }

    #[test]
    fn test_manager_set_desired_allocates_monotonic_dap_ids() {
        let mut mgr = BreakpointManager::new();
        let bps = mgr.set_desired("file:///main.dart", &[bp_line(10), bp_line(20)]);
        assert_eq!(bps[0].dap_id, 1);
        assert_eq!(bps[1].dap_id, 2);
    }

    #[test]
    fn test_manager_clear_active_removes_all_vm_entries() {
        let mut mgr = BreakpointManager::new();
        // Record some active breakpoints.
        mgr.record_active(
            1,
            "bp/1",
            "file:///main.dart",
            Some(10),
            None,
            true,
            BreakpointCondition::default(),
        );
        mgr.record_active(
            2,
            "bp/2",
            "file:///main.dart",
            Some(20),
            None,
            true,
            BreakpointCondition::default(),
        );

        assert_eq!(mgr.active().len(), 2);
        let cleared = mgr.clear_active();
        assert_eq!(cleared.len(), 2, "Should return all cleared entries");
        assert_eq!(
            mgr.active().len(),
            0,
            "Active state must be empty after clear"
        );
    }

    #[test]
    fn test_manager_desired_iter_yields_all_uris() {
        let mut mgr = BreakpointManager::new();
        mgr.set_desired("file:///a.dart", &[bp_line(1)]);
        mgr.set_desired("file:///b.dart", &[bp_line(2), bp_line(3)]);

        let all: Vec<_> = mgr.desired_iter().collect();
        assert_eq!(
            all.len(),
            3,
            "Should yield 3 desired breakpoints across 2 URIs"
        );
    }

    #[test]
    fn test_manager_conditional_breakpoint_preserved_through_restart() {
        let mut mgr = BreakpointManager::new();
        let bps = mgr.set_desired("file:///main.dart", &[bp_with_cond(10, "x > 5")]);
        let dap_id = bps[0].dap_id;

        mgr.clear_active();

        let desired = mgr.desired_for("file:///main.dart");
        assert_eq!(desired.len(), 1);
        assert_eq!(
            desired[0].dap_id, dap_id,
            "DAP ID must be stable after restart"
        );
        assert_eq!(
            desired[0].condition.as_deref(),
            Some("x > 5"),
            "Condition must survive restart"
        );
    }

    #[test]
    fn test_manager_logpoint_message_preserved_through_restart() {
        let mut mgr = BreakpointManager::new();
        let bps = mgr.set_desired("file:///main.dart", &[bp_with_log(15, "x = {x}")]);

        mgr.clear_active();

        let desired = mgr.desired_for("file:///main.dart");
        assert_eq!(desired.len(), 1);
        assert_eq!(bps[0].dap_id, desired[0].dap_id);
        assert_eq!(
            desired[0].log_message.as_deref(),
            Some("x = {x}"),
            "Log message must survive restart"
        );
    }

    #[test]
    fn test_manager_no_duplicate_breakpoints_after_multiple_restarts() {
        let mut mgr = BreakpointManager::new();
        mgr.set_desired("file:///main.dart", &[bp_line(25)]);

        // Simulate 3 restart cycles.
        for i in 0..3u32 {
            mgr.clear_active();
            let vm_id = format!("bp/vm-{}", i);
            mgr.record_active(
                mgr.desired_for("file:///main.dart")[0].dap_id,
                vm_id,
                "file:///main.dart",
                Some(25),
                None,
                true,
                BreakpointCondition::default(),
            );
        }

        // Only one active breakpoint should exist (no duplicates).
        assert_eq!(
            mgr.active().len(),
            1,
            "Only 1 active breakpoint should exist, not duplicates after multiple restarts"
        );
    }

    #[test]
    fn test_manager_record_active_links_desired_dap_id_to_vm_id() {
        let mut mgr = BreakpointManager::new();
        let bps = mgr.set_desired("file:///main.dart", &[bp_line(42)]);
        let dap_id = bps[0].dap_id;

        mgr.record_active(
            dap_id,
            "bp/vm-42",
            "file:///main.dart",
            Some(42),
            None,
            true,
            BreakpointCondition::default(),
        );

        let entry = mgr
            .lookup_active_by_dap_id(dap_id)
            .expect("Entry must exist");
        assert_eq!(entry.dap_id, dap_id);
        assert_eq!(entry.vm_id, "bp/vm-42");
        // Also findable by VM ID.
        let by_vm = mgr
            .lookup_active_by_vm_id("bp/vm-42")
            .expect("Should find by VM ID");
        assert_eq!(by_vm.dap_id, dap_id);
    }

    #[test]
    fn test_manager_desired_dap_ids_are_stable_across_set_desired_calls() {
        let mut mgr = BreakpointManager::new();
        // First call allocates IDs 1 and 2.
        let bps1 = mgr.set_desired("file:///main.dart", &[bp_line(10), bp_line(20)]);
        let id1 = bps1[0].dap_id;
        let id2 = bps1[1].dap_id;

        // Second call to set_desired for the same URI allocates fresh IDs (3 and 4).
        let bps2 = mgr.set_desired("file:///main.dart", &[bp_line(10), bp_line(20)]);
        // New IDs are allocated; old desired is replaced.
        assert_ne!(bps2[0].dap_id, id1, "Second set_desired allocates new IDs");
        assert_ne!(bps2[1].dap_id, id2);
    }

    #[test]
    fn test_manager_desired_uris_returns_all_source_files() {
        let mut mgr = BreakpointManager::new();
        mgr.set_desired("file:///lib/main.dart", &[bp_line(5)]);
        mgr.set_desired("file:///lib/home.dart", &[bp_line(10)]);

        let uris: std::collections::HashSet<&str> = mgr.desired_uris().collect();
        assert!(uris.contains("file:///lib/main.dart"));
        assert!(uris.contains("file:///lib/home.dart"));
        assert_eq!(uris.len(), 2);
    }

    #[test]
    fn test_manager_clear_desired_for_uri_removes_only_that_uri() {
        let mut mgr = BreakpointManager::new();
        mgr.set_desired("file:///lib/main.dart", &[bp_line(5)]);
        mgr.set_desired("file:///lib/home.dart", &[bp_line(10)]);

        let removed = mgr.clear_desired_for_uri("file:///lib/main.dart");
        assert_eq!(removed.len(), 1);
        assert!(mgr.desired_for("file:///lib/main.dart").is_empty());
        assert_eq!(mgr.desired_for("file:///lib/home.dart").len(), 1);
    }
}
