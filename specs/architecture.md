# Architecture

## Shape

Use a Rust workspace with one desktop application and independently testable domain/integration modules. Keep telemetry acquisition, presence publishing, UI, configuration, and formatting behind narrow interfaces so the mock source exercises the same path as NOVA.

```text
GUI / tray
    │ commands + snapshots
Application state / polling coordinator
    ├── TelemetrySource: Mock | NOVA HTTP client
    ├── Optional CompanionSource: bounded authenticated LAN protocol | mock peer
    ├── PresencePublisher: preview | Discord local IPC
    ├── SettingsStore: config + OS credential store
    └── diagnostics (redacted)
```

## Modules

- `domain`: typed telemetry, connection state, availability, settings, title identity.
- `telemetry::nova`: REST client, optional JWT authentication, endpoint decoding, LAN discovery, retry/backoff.
- `telemetry::mock`: deterministic sample and fault injection for development and UI preview.
- `companion`: optional host-side listener and versioned message decoder for a title-scoped Peer Pressure DLL. The first implementation is protocol + mock only; it does not build, install, or deploy an Xbox DLL.
- `presence`: pure formatter and Discord RPC client. IPC uses the official handshake and `SET_ACTIVITY` frame over Unix sockets or Windows named pipes. It accepts only a Discord Application ID and never handles user credentials.
- `catalog`: offline Title ID to display name and Discord artwork key lookup for the initial curated set (Halo 3, Halo: Reach, Halo Wars, Halo 4, Grand Theft Auto V, and Skyrim). Unknown titles use raw IDs and neutral art. Asset keys require matching artwork uploads in the user's Discord application.
- `settings`: persistent non-secret preferences and credential-store adapter.
- `ui`: native cross-platform window, status, preview, controls, diagnostics, tray lifecycle.

## Runtime flow

1. Load preferences and retrieve secrets from the OS credential store.
2. Select configured source: NOVA, companion, or mock. NOVA remains the compatible default live source; companion mode is opt-in.
3. Connect/authenticate to NOVA and poll at a configurable interval, or accept bounded companion messages on a user-selected LAN listener.
4. Decode each field independently into `Some(value)` or unavailable; retain last-known values only for transient display diagnostics, never re-label stale values as live.
5. Update UI snapshot and format presence from available fields and user toggles.
6. Connect to Discord IPC when the desktop client is present; publish or clear presence on state transitions.
7. On title change, set a new elapsed-session start timestamp. On disconnect, clear presence after a short grace period and display disconnected status.

## Platform and dependency choices

The implementation uses eframe/egui with glow and X11/Wayland backends for its native desktop window. Linux tray support uses StatusNotifier via `ksni`; Windows/macOS currently lack tray controls and closing those windows exits. The NOVA transport uses bounded `std::net` HTTP over an explicitly configured IP address and port; authentication follows NOVA's multipart `/authenticate` request and bearer JWT flow. This avoids a large HTTP dependency while keeping requests local and time-bounded. Tray support is isolated in the UI module because platform packaging differs.

## Optional Xbox companion feasibility

Peer Pressure publicly documents DLL sideloading on a per-title basis. Its repository does not publish implementation source or a telemetry API. Therefore an optional companion is a game-scoped executable, not an always-running service: when no configured title DLL is loaded (dashboard, unsupported title, transition, or process exit), desktop telemetry must go stale and the timer/presence must clear according to source policy. Never infer persistent execution from Peer Pressure being a persistent softmod.

The first milestone implements only the host-side protocol and a mock peer. Protocol v1 is TCP initiated by the companion to a host-selected private-LAN address/port. Frames start with a 4-byte unsigned big-endian payload length (1..4096), followed by UTF-8 JSON. Server sends a fresh random 32-byte challenge on accept. Companion returns a response with `version`, `type=authenticate`, and HMAC-SHA256 proof over the domain string `XeRust360-RPC companion auth v1` concatenated with the raw challenge, using a user-configured 32-byte shared key. After acceptance, each companion message has `version`, `type=telemetry`, strictly increasing `sequence` (u64), allowlisted optional telemetry fields, and HMAC-SHA256 over domain string `XeRust360-RPC companion telemetry v1`, challenge, big-endian sequence, and the exact UTF-8 JSON object bytes with the `mac` property omitted. To avoid JSON canonicalization ambiguity, v1 field order and compact serialization are fixed in the codec and tested against golden vectors; unknown fields are rejected. No commands from desktop to console are allowed after the authentication exchange. Reject malformed, stale, oversized, unauthenticated, or unsupported-version frames. Desktop receive time (not companion clock) establishes freshness; a missed heartbeat timeout ends the title session and clears values. The companion sends only observed values; absent values stay absent.

There is no public stable source for title-wide achievements, signed-in profile, gamerscore, or temperature in a per-game Peer Pressure DLL. These remain unavailable until validated against a documented API and real hardware. A title ID can be explicitly configured for a title-specific build, but do not present a configured ID as proof the title is currently running. A real game DLL requires a compatible Xbox 360 XEX/DLL build chain (Xenon PowerPC Microsoft ABI, imports, and packaging); libXenon is a bare-metal toolchain and is not sufficient evidence. No memory patching, kernel hooks, or automatic deployment are in scope.

## Network and trust boundaries

- NOVA traffic stays on the configured LAN host; bounded discovery probes only private/local interfaces.
- Optional NOVA authentication returns a short-lived token held in memory. The password remains in the OS credential store.
- Discord IPC connects only to a local Discord process. No user token, Discord gateway, or external presence service exists.
- Logs redact credentials, tokens, and sensitive console identifiers.

NOVA schema mapping follows the published `0.7b.2 r1622` API: `/title.titleid`; signed-in `/profile` entries with `index`, `gamertag`, and `gamerscore`; `/achievement` entries with `id` and `cred`; `/achievement/player` entries matched by `id` and read at the signed-in profile slot; and `/temperature` `cpu`, `gpu`, and `celsius` fields. Fahrenheit sensor values are converted to Celsius. Missing profile slot or unit information leaves those values unavailable.

## Failure behavior

Network errors, unauthorized responses, unsupported fields, absent Discord IPC, absent profiles, and unknown Title IDs are expected states. The coordinator reports them to GUI/diagnostics and retries only where useful. Parsing errors for one field must not invalidate unrelated fields.
