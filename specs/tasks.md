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
- [ ] Emit an explicit SKIPPED reason when local policy denies loopback sockets, while requiring loopback in CI.
- [ ] Run authenticated session/disconnect, malformed-message, and heartbeat-timeout TCP tests in Linux/Windows/macOS CI with localhost required.
- Acceptance: desktop flow is testable entirely with mock peer; no companion message can issue console commands; existing NOVA behavior remains covered.

## M7 — Peer Pressure loader/toolchain feasibility

- [x] Research official Peer Pressure sideload INI precedence, matching fields, load timing, title DLL type, and unload-on-game-exit behavior.
- [x] Compare XDK, OXDK, libXenon, xenosworks/xsdk, and RB3Enhanced's real-world DLL build instructions/artifacts.
- [x] Review the commonly referenced XAM current-title export; classify it as reverse-engineered and not yet a supported/validated telemetry API.
- [ ] Build a harmless title-DLL skeleton using a compatible XDK installation; record the binary format, entrypoint, imports, and static verification results separately from loader compatibility. **Not runnable here: no XDK toolchain detected.**
- [ ] Verify the built DLL is actually accepted and unloaded by Peer Pressure on hardware; this app must not deploy it.
- [ ] Identify documented safe runtime APIs for Title ID/heartbeat. Do not implement socket/thread work in DLL entrypoint before runtime initialization constraints are established.
- [ ] Validate title launch/exit, unload, network loss, and supported hardware on a separately arranged test setup. No console deployment is performed by this application.

## M8 — Host companion simulation

- [ ] Add `xbox-companion/` host simulator using the existing authenticated companion protocol.
- [ ] Send periodic heartbeat-only telemetry plus optional explicitly configured candidate Title ID on one session; label it as simulation input.
- [ ] Add simulator build scripts/prerequisites and keep XDK proprietary files external.
- [ ] Add a protocol integration test for multiple heartbeat frames on one connection and build the simulator in cross-platform CI.
- Acceptance: host simulator compiles and uses protocol v1; no Xbox binary or claim of Peer Pressure compatibility is produced.
- [ ] Measure which telemetry fields have documented and stable APIs; enable them only after verification.
- Acceptance: hardware results and unverified cases are recorded; no game-wide support claim without evidence.

## M9 — Dynamic title metadata and artwork

- [x] Remove the six-title hardcoded name/artwork catalog while preserving exact-ID unknown behavior.
- [x] Add provider-neutral Xbox 360 metadata/artwork types and exact Title ID normalization.
- [x] Implement x360db metadata/boxart lookup and XboxUnity cover fallback with source labels and official-cover preference.
- [x] Add bounded timeouts, low request rates, cache size/TTL policy, negative caching, and offline cache resolution.
- [x] Disclose that online lookup sends only the active Title ID to third-party providers; tolerate missing system `curl` by falling back to cache/generic art.
- [x] Pass validated provider HTTPS cover URLs through Discord local RPC `assets.large_image`; use generic uploaded Xbox 360 image when unavailable.
- [x] Update UI preview and diagnostics to distinguish live remote URL, cached local artwork, generic fallback, unknown title, and provider failure.
- [x] Add fixture/mock tests for real IDs, alternate/multi-disc IDs, missing artwork, provider failure/fallback, cache hit/expiry, offline resolution, throttling, and malformed URLs. Live provider checks remain blocked by this environment's DNS restriction.
- [x] Update CI, README, compatibility notes, and asset setup guidance. Do not bundle third-party cover files.
- Acceptance: real documented Title IDs resolve through provider fixtures; no unknown ID is assigned a game; actual remote artwork is represented by an HTTPS URL; Discord rendering is described as protocol-supported but not live-verified without a client check.

## M10 — Artwork end-to-end validation and diagnostics

- [x] Add diagnostics for observed Title ID, exact resolved title, provider, public image URL, cache status, and fallback reason; never expose credentials or raw provider bodies.
- [x] Add controlled tests for redirects, content type/image signature, cache reuse, rate limits, and provider outages.
- [ ] Run opt-in live x360db and XboxUnity checks for representative disc, XBLA, obscure, and multi-disc records; document Original Xbox coverage only when provider metadata explicitly labels it.
- [ ] Record actual HTTP/DNS outcomes, exact matching, and cache results; blocked network is not a passing provider result.
- [ ] Verify external HTTPS artwork with official local IPC and a running Discord desktop client; confirm visible image rendering. IPC payload acceptance alone is insufficient.
- Acceptance: fixture/controlled cases pass, and live provider/client checks are separately evidenced or explicitly blocked. No identity, artwork availability, or rendering success is guessed.

## M11 — Operator validation workflow

- [x] Add opt-in `--artwork-diagnostics <TITLE_ID>` CLI mode for live per-provider metadata and cover checks, with HTTP status, redirects/final HTTPS URL, MIME/signature, cache behavior, and honest nonzero errors.
- [x] Add opt-in `--discord-test <TITLE_ID> [--duration-seconds N]` using resolved live provider artwork and configured Application ID only; correlate the Discord RPC response with the request nonce and clear activity when done.
- [x] Add parser/transport/response tests without default network or desktop dependencies; keep separate visible-render verification manual.
- [x] Add manual Linux workflow and checklist; run unit and compile checks in existing Linux/Windows/macOS CI matrix without external provider calls.
- Acceptance: live modes never use or label mock data; failures are explicit; IPC acknowledgment is distinct from visually rendered artwork; local IPC remains tokenless and no unsafe Xbox operations are added.

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
| T-20 | TCP peer closes after valid telemetry | Detect EOF as immediate session disconnect; do not wait for heartbeat timeout. |
| T-21 | Authenticated peer sends malformed telemetry frame | Reject the frame, end the session, and preserve the last trusted snapshot until the coordinator clears the disconnected source. |
| T-22 | Host denies localhost bind | Test prints an explicit `SKIPPED` reason locally; CI's required-loopback mode turns this into a failure. |
| T-23 | CI matrix localhost integration | All three TCP scenarios execute successfully on Linux, Windows, and macOS; any bind/connect/test failure fails the job. |
| T-24 | Title-DLL source build | A reproducible XDK build produces a structurally validated title DLL; this is build evidence only, not Peer Pressure loader/hardware compatibility. |
| T-25 | Host companion simulation | Simulator authenticates once, sends monotonic heartbeat telemetry with only an optional supplied Title ID, and lets desktop freshness/disconnect behavior apply. |
| T-26 | Known Title ID metadata | Provider fixtures resolve Halo IDs `4D5307E6`, `4D53085B`, and XBLA Minecraft ID `584111F7` to Title ID-declared names and cover URLs where available. |
| T-27 | Unknown Title ID | No provider match yields raw unresolved Title ID and generic Xbox 360 cover, without inferred identity. |
| T-28 | Provider fallback | Primary timeout/not-found uses the next provider; malformed or non-HTTPS image URLs are rejected. |
| T-29 | Metadata/art cache | Repeated lookup is cached; expired entries refresh; offline lookup returns cached data or unavailable; cache remains bounded. |
| T-30 | Discord external image URL | RPC `SET_ACTIVITY` includes the cover URL exactly when present and the generic asset when absent. This validates serialization, not real-client rendering. |
| T-31 | Provider throttling and failure | Repeated lookup obeys per-provider limits; provider errors do not block UI or telemetry updates. |
