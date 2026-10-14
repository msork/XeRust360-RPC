# Tasks and milestones

Every milestone is committed separately after its acceptance checks. Update these tasks and the relevant specs as implementation decisions change.

## M0 — Research and specification (complete before code)

- [x] Review XeCord, NOVA API documentation, BadUpdate/ABadAvatar context, Discord Rich Presence docs, and supplied Reddit link availability.
- [x] Research Peer Pressure's public docs, title-scoped DLL sideloading, hardware compatibility, public source/API availability, and Xenon toolchain constraints.
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

## M6 — Optional Peer Pressure companion protocol (mock-first)

- [x] Define versioned, bounded, authenticated read-only LAN protocol, freshness/lifecycle semantics, and metric availability contract in all applicable specs before implementation.
- [x] Implement mock companion sender, desktop listener/source adapter, source selection, stale-session clearing, and diagnostics without Xbox binary or deployment support.
- [x] Add protocol tests for valid fields, malformed/oversized/stale/unauthenticated/version-mismatch frames, lifecycle stop, and absence preservation.
- [ ] Run socket integration tests on an unsandboxed runner; current sandbox denies loopback TCP binds, so those tests are skipped locally.
- Acceptance: desktop flow is testable entirely with mock peer; no companion message can issue console commands; existing NOVA behavior remains covered.

## M7 — Hardware feasibility and compatibility validation (blocked on equipment/toolchain)

- [x] Research public toolchains: libXenon is bare-metal; OXDK documents Xenon ABI/XEX title-app builds, not Peer Pressure DLL sideloading.
- [x] Confirm Peer Pressure publicly documents title DLL sideloading but no loader ABI, source, telemetry API, or explicit game DLL lifecycle contract.
- [ ] Verify a supported Xenon game ABI/XEX-DLL toolchain and Peer Pressure loader contract without modifying console boot/NAND or using hooks. **Gate not met; do not create a DLL prototype.**
- [ ] Prototype only after static review and mock protocol pass; no automatic deployment.
- [ ] Validate a minimal read-only companion on real hardware, including game launch/exit, unload, network loss, and supported motherboard/boot method. No deployment or hardware operation is performed by this application.
- [ ] Measure which telemetry fields have documented and stable APIs; enable them only after verification.
- Acceptance: hardware results and unverified cases are recorded; no game-wide support claim without evidence.

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
| T-13 | Companion sends valid title-scoped record | Only provided fields enter telemetry; session timer starts from fresh observation. |
| T-14 | Companion exits or heartbeat expires | Session goes unavailable, stale values clear, presence is cleared, and next game gets a new timer. |
| T-15 | Invalid, oversized, stale, unauthenticated, or wrong-version companion frame | Reject frame without changing current trusted telemetry or crashing. |
| T-16 | Companion record omits achievements/profile/temperature | Those values remain absent; no fallback to guessed values. |
| T-17 | NOVA selection while companion absent | Existing NOVA integration works unchanged. |
| T-18 | Xbox artifact feasibility gate | Require evidence for PPC game ABI, imports, XEX/DLL packaging, Peer Pressure entrypoint, and teardown before any binary is added. |
| T-19 | Companion title transition/exit | Treat process/title termination as disconnect, expire all values, clear presence, and start a new timer only on fresh authenticated data. |
