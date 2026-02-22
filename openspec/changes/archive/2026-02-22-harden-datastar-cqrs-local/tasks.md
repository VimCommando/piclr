## 1. Command Path Locking and CQRS Transport

- [x] 1.1 Refactor immediate-apply command flow in `/Users/reno/Development/piclr/src/web/mod.rs` to avoid holding the state write lock across filesystem `await` calls.
- [x] 1.2 Preserve undo semantics and patch emission order after lock-scope refactor, including single-entry stack patch behavior.
- [x] 1.3 Migrate stream routing from `PATCH /events` to `GET /events` in router definitions and Datastar init wiring.

## 2. Stream Consistency Recovery

- [x] 2.1 Add lag handling logic in the `/events` stream loop that emits a full `UiPatch::ALL` resync for lagged subscribers before continuing incremental events.
- [x] 2.2 Validate bootstrap and resync patch construction paths remain consistent with authoritative server read-model generation.
- [x] 2.3 Review broadcast channel sizing and document rationale for chosen capacity in the local co-located deployment model.

## 3. Frontend Command Dispatch Reliability

- [x] 3.1 Update keyboard shortcut `@post('/cmd/*')` actions in `/Users/reno/Development/piclr/templates/index.html` to use Datastar options that disable auto-cancellation for rapid command bursts.
- [x] 3.2 Keep vanilla JavaScript helper surface minimal while preserving Tauri directory-picker compatibility paths.
- [x] 3.3 Verify modal and button command actions still behave correctly under the updated request cancellation policy.

## 4. Regression Tests and Verification

- [x] 4.1 Add or extend tests covering stream lag recovery so a lag event results in full view resync patches.
- [x] 4.2 Add or extend tests that validate rapid keyboard command dispatch is not dropped by cancellation defaults.
- [x] 4.3 Run full test suite and targeted manual validation for Tauri-wrapper behavior (`Ctrl+O`, command shortcuts, modal interactions).
