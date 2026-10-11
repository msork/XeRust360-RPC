# Tasks and milestones

Every milestone is committed separately after its acceptance checks. Update these tasks and the relevant specs as implementation decisions change.

## M0 — Research and specification (complete before code)

- [x] Review XeCord, NOVA API documentation, BadUpdate/ABadAvatar context, Discord Rich Presence docs, and supplied Reddit link availability.
- [x] Define product requirements, architecture, GUI design, tasks, and test cases.
- Acceptance: all five spec files exist and research constraints are explicit.

## M1 — Rust project foundation and domain model

- [x] Initialize Cargo project/workspace, app entry point, typed telemetry/settings/presence models, formatting helpers, and mock data.
- [x] Add core unit tests for missing values, optional state components, timer reset, and UTF-8-safe limits.
- Acceptance: Linux `cargo test` and `cargo check` pass; app can generate a mock preview.

## M2 — NOVA telemetry integration

- [x] Implement configurable HTTP client, optional auth, endpoint parsing, bounded private-LAN discovery, polling, reconnect/backoff schedule, and redacted diagnostics.
- [x] Add schema-shaped response tests for NOVA values, partial fields, HTTP chunking/204, and endpoint/auth error redaction.
- Acceptance: each telemetry field parses independently; invalid/missing fields are marked unavailable.

## M3 — Discord RPC integration

- [x] Implement Discord IPC path detection, handshake, activity update/clear, retry-ready client behavior, and payload validation.
- [x] Verify RPC text limits against official Discord SDK field docs and enforce at the formatter boundary.
- Acceptance: local IPC only; no gateway/user token code; disconnected Discord is a recoverable state.

## M4 — Native UI, settings, tray, and credentials

- [x] Implement dashboard, live preview, toggles, status/errors, diagnostics, persistent preferences, OS keyring integration, and Linux tray behavior.
- [x] Exercise compilation and automated UI-supporting tests with mock source and absent Discord/NOVA.
- [ ] Add native tray integration on Windows and macOS.
- Acceptance: primary flows usable without hardware; secrets never serialized into ordinary config/logs. Linux has tray controls; Windows/macOS tray remains open.

## M5 — Cross-platform CI and release readiness

- [x] Add CI matrix for Linux, Windows, macOS; document setup, NOVA prerequisites, Discord application assets, security, and known gaps.
- [x] Check format, tests, lint/build workflow where supported.
- Acceptance: CI configuration covers all target OSes; native Linux checks pass locally.

## Milestone test cases

| ID | Case | Expected result |
|---|---|---|
| T-01 | NOVA fixture has all telemetry | All available values represented with correct units/IDs. |
| T-02 | NOVA omits profile/achievement/temp fields | Missing values remain absent and are not rendered as zero. |
| T-03 | Unknown Title ID | Details show raw ID; generic art is used. |
| T-04 | Change running Title ID | Elapsed timer restarts at the observed change. |
| T-05 | Toggle each State component | Only enabled, available component is emitted in defined order. |
| T-06 | Long Unicode fields | State/details fit configured byte cap without invalid UTF-8. |
| T-07 | NOVA timeout then recovery | Status enters reconnecting, retries with bounded backoff, then recovers. |
| T-08 | NOVA unauthorized | Clear auth diagnostic; no secret in logs. |
| T-09 | Discord absent then starts | Presence is queued/refreshed after IPC becomes available. |
| T-10 | Discord IPC path unavailable | App remains usable and reports Discord disconnected. |
| T-11 | Settings save/reload | Non-secret settings persist; password goes through credential adapter. |
| T-12 | Mock mode on each OS | Preview works without console/Discord connection. |
