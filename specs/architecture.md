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
- `catalog`: provider-independent exact Title ID normalization and metadata types.
- `artwork`: provider chain, metadata/art cache, request throttling, and URL validation. Initial providers are x360db for broad metadata/boxart, then XboxUnity's documented community cover API as a fallback. Provider fetches are bounded and metadata is cached locally. Keep the returned remote URL for Discord and cached image bytes for local preview. Do not equate cache availability with Discord URL reachability.
- Carry artwork diagnostic fields across the worker/UI boundary: normalized observed Title ID, exact resolved title, provenance, public image URL, cache outcome, and fallback reason. Never surface credentials, headers, or raw provider response bodies.
- `settings`: persistent non-secret preferences and credential-store adapter.
- `ui`: native cross-platform window, status, preview, controls, diagnostics, tray lifecycle.

## Runtime flow

1. Load preferences and retrieve secrets from the OS credential store.
2. Select configured source: NOVA, companion, or mock. NOVA remains the compatible default live source; companion mode is opt-in.
3. Connect/authenticate to NOVA and poll at a configurable interval, or accept bounded companion messages on a user-selected LAN listener.
4. Decode each field independently into `Some(value)` or unavailable; retain last-known values only for transient display diagnostics, never re-label stale values as live.
5. Resolve metadata by exact observed Title ID through the artwork provider chain; consult the local cache first, then rate-limited providers. Cache metadata and bounded image bytes, retain provenance, and select the first valid remote HTTPS cover URL.
6. Update UI snapshot and format presence from available fields and user toggles. Set `assets.large_image` to the remote cover URL only when one is available; otherwise use the generic Xbox 360 Discord asset. The UI preview may use cached bytes while offline.
7. Connect to Discord IPC when the desktop client is present; publish or clear presence on state transitions.
8. On title change, set a new elapsed-session start timestamp. On disconnect, clear presence after a short grace period and display disconnected status.

The Xbox 360 artwork lookup order is exact Title ID only. x360db is the primary community metadata index (documented ~6,000 entries, alternate IDs, media IDs, and boxart field). Its media IDs can describe disc variants, but Title ID-only runtime telemetry cannot distinguish them. XboxUnity is an optional runtime fallback with official/community cover flags. GameTDB is excluded because a verifiable Xbox 360 catalog was not established. Use HTTPS, explicit timeouts, low per-provider request rates, negative-result TTLs, cache size limits, and no background bulk scraping. The bounded HTTPS adapter invokes system `curl`; if it is unavailable, lookup fails closed to cache/generic art. Online lookup sends only the current Title ID to the configured artwork providers. Do not package third-party images: x360db's cover license is unclear, and XboxUnity's archive license excludes original artwork. Discord's official RPC-over-IPC `SET_ACTIVITY` supports activity assets and official Rich Presence docs support external image URLs; URLs are passed through, not proxied by this app. This is protocol evidence only until the configured live Discord client visibly renders an image.

## Platform and dependency choices

The implementation uses eframe/egui with glow and X11/Wayland backends for its native desktop window. Linux tray support uses StatusNotifier via `ksni`; Windows/macOS currently lack tray controls and closing those windows exits. The NOVA transport uses bounded `std::net` HTTP over an explicitly configured IP address and port; authentication follows NOVA's multipart `/authenticate` request and bearer JWT flow. This avoids a large HTTP dependency while keeping requests local and time-bounded. Tray support is isolated in the UI module because platform packaging differs.

## Optional Xbox companion feasibility

Peer Pressure publicly documents DLL sideloading on a per-title basis. Its repository does not publish implementation source or a telemetry API. Therefore an optional companion is a game-scoped executable, not an always-running service: when no configured title DLL is loaded (dashboard, unsupported title, transition, or process exit), desktop telemetry must go stale and the timer/presence must clear according to source policy. Never infer persistent execution from Peer Pressure being a persistent softmod.

The first milestone implements only the host-side protocol and a mock peer. Protocol v1 is TCP initiated by the companion to a host-selected private-LAN address/port. Frames start with a 4-byte unsigned big-endian payload length (1..4096), followed by UTF-8 JSON. Server sends a fresh random 32-byte challenge on accept. Companion returns a response with `version`, `type=authenticate`, and HMAC-SHA256 proof over the domain string `XeRust360-RPC companion auth v1` concatenated with the raw challenge, using a user-configured 32-byte shared key. After acceptance, each companion message has `version`, `type=telemetry`, strictly increasing `sequence` (u64), allowlisted optional telemetry fields, and HMAC-SHA256 over domain string `XeRust360-RPC companion telemetry v1`, challenge, big-endian sequence, and the canonical compact JSON encoding of the unsigned telemetry struct. Canonical encoding uses fixed struct field order, includes every optional telemetry field as a key with `null` when absent, uses no whitespace, rejects unknown/duplicate fields, and is covered by golden vectors. The signed `mac` field is appended last in the wire object. No commands from desktop to console are allowed after the authentication exchange. Reject malformed, stale, oversized, unauthenticated, or unsupported-version frames. Desktop receive time (not companion clock) establishes freshness; a missed heartbeat timeout ends the title session and clears values. The companion sends only observed values; absent values stay absent.

Peer Pressure's official wiki specifies lookup order (`<game xex directory>\\sideload.ini`, `<game xex directory>\\<title id>.ini`, then `Hdd1:\\PeerPressure\\SideLoading\\<title id>.ini`), filters by title/media/version, and loads a configured title DLL after mapping the game XEX but before its entrypoint/CRT initialization. It says the DLL is unloaded when the game exits and must not hook outside game XEX files. It does not publish its loader source or the low-level import resolution/ABI details.

The credible build path for a title DLL is the Xbox 360 XDK: RB3Enhanced's real-world Makefile compiles with XDK `cl.exe`, links a DLL with `_DllMainCRTStartup` and XDK libraries, then wraps it using `imagexex.exe`; its build guide says Wine can invoke the SDK tools on Linux/macOS. This is evidence of producing an Xbox 360 title DLL, not evidence that our DLL is accepted by Peer Pressure. OXDK documents an alternate Xenon ABI/XEX title toolchain but no demonstrated title-DLL packaging path. The newer open-source `xenosworks/xsdk` documents title XEX builds and hardware-tested runtime milestones, but its published build interface is title XEX-only. libXenon remains bare-metal and unsuitable for this use. The XDK toolchain is not installed in this environment, so no artifact has been built. No source artifact should be called Peer Pressure-compatible without target-hardware loading evidence.

There is no public stable source for title-wide achievements, signed-in profile, gamerscore, or temperature in a per-game Peer Pressure DLL. These remain unavailable until validated against a documented API and real hardware. A configured Title ID is not proof that the title is currently running. RB3Enhanced's build records the conventional `_DllMainCRTStartup` entrypoint, but Peer Pressure does not publish exact import-resolution details for its loader. Reverse-engineered XAM ordinal tables and public mods identify `XamGetCurrentTitleId` at ordinal `0x1CF`, but no authoritative public Xbox 360 API contract or compatibility range was found; it is not an approved data source for this app yet. A future prototype could observe process attach/detach and send heartbeat only after CRT/runtime-safe initialization; safe socket/thread APIs in DLL entrypoint have not been established. No memory patching, kernel hooks, system hooks, NAND writes, or automatic deployment are in scope.

Until a compatible XDK installation and supported Title ID API are available, `xbox-companion/` contains a host-only simulator that reuses the desktop authenticated protocol. It sends periodic heartbeat telemetry and may include a user-provided candidate Title ID; that value is configuration/simulation input only. The simulator is not built for Xenon and does not establish XEX DLL loader compatibility. Its separate documentation records the proprietary XDK prerequisites and the future checks needed for safe DLL startup, networking, and teardown.

Treat companion execution as title-scoped and potentially absent at every transition. The desktop connection and heartbeat define freshness; title exit, dashboard transition, or process loss expires all values and clears presence. A future artifact must establish a documented load entrypoint and teardown contract first. Heartbeat and runtime-observed Title ID are candidates, not currently proven telemetry.

## Network and trust boundaries

- NOVA traffic stays on the configured LAN host; bounded discovery probes only private/local interfaces.
- Optional NOVA authentication returns a short-lived token held in memory. The password remains in the OS credential store.
- Discord IPC connects only to a local Discord process. No user token, Discord gateway, or external presence service exists.
- Logs redact credentials, tokens, and sensitive console identifiers.

NOVA schema mapping follows the published `0.7b.2 r1622` API: `/title.titleid`; signed-in `/profile` entries with `index`, `gamertag`, and `gamerscore`; `/achievement` entries with `id` and `cred`; `/achievement/player` entries matched by `id` and read at the signed-in profile slot; and `/temperature` `cpu`, `gpu`, and `celsius` fields. Fahrenheit sensor values are converted to Celsius. Missing profile slot or unit information leaves those values unavailable.

## Failure behavior

Network errors, unauthorized responses, unsupported fields, absent Discord IPC, absent profiles, and unknown Title IDs are expected states. The coordinator reports them to GUI/diagnostics and retries only where useful. Parsing errors for one field must not invalidate unrelated fields.
