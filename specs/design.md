# Design

## Desktop experience

The main window is a compact dashboard with a title bar, console connection section, Rich Presence preview card, settings section, and diagnostics drawer. The UI uses a restrained dark palette with clear text contrast and Xbox green as a small accent. The preview mirrors Discord's line order and updates from either live data or mock mode.

### Main view

- Header: XeRust360-RPC name/icon, running state, and menu.
- Console: host, Discover, Connect/Disconnect, and status (Searching, Connected, Reconnecting, Unavailable).
- Telemetry source: NOVA, optional Xbox companion, or mock. NOVA remains the supported default for compatible consoles; companion status must name the configured title/session and distinguish “no companion process” from console/network failure.
- Discord: detected/connected status and presence publish status.
- Preview: “Playing Xbox 360”, game in Details, composed State, elapsed time, large artwork and small project icon.
- Statistics: per-field toggles for gamertag, achievements, game GS, total GS, CPU, and GPU.
- Settings: host/port, NOVA username/password, polling interval, auto-reconnect, mock mode, and launch-at-login only if support is reliable on all targets.
- Diagnostics: timestamps, endpoint status, sanitized error, and copy diagnostics action.

## Presence layout

- Activity name: `Xbox 360` (Discord application registration must match the project-owned application name).
- Details: resolved game name; unresolved values display `Title ID XXXXXXXX`.
- State: join enabled components with ` // `, preserving the requested ordering. Examples: `Player // 5/50 achievements // 120 GS // 24000 GS // CPU 55°C / GPU 61°C`.
- Timestamp: start time recorded when the observed Title ID changes.
- Large image: exact Title ID provider's public HTTPS cover URL when available; otherwise generic Xbox 360 placeholder asset key. UI preview may show locally cached bytes while offline.
- Small image: project icon key.

Discord's official Rich Presence SDK documents text fields with a maximum of 128 characters. This implementation caps details/state at 128 UTF-8 bytes, which is stricter for multibyte text, and truncates only at character boundaries. The RPC adapter should preserve this limit and surface truncation in diagnostics.

## Accessibility and behavior

- All controls have text labels; status is represented by text and color.
- Missing data is omitted from State and shown as “Unavailable” in the diagnostics/UI field row, never as zero.
- Credentials use password controls and are never included in previews, logs, or copied diagnostics.
- Tray menu offers Show, Pause/Resume presence, and Quit. Closing the window hides to tray only when tray support is available; otherwise it quits or minimizes according to platform convention.
- On first launch, mock mode is discoverable and can produce a safe local preview without a console.

## Optional Peer Pressure companion

The companion option is an expert opt-in. The desktop app opens a bounded LAN listener and displays its address, selected port, session status, and last valid message. It does not write files to the Xbox, edit Peer Pressure configuration, sideload, or deploy a DLL. Users must independently build and configure a title-specific read-only DLL through supported Peer Pressure mechanisms. When that game exits or stops sending fresh telemetry, the app clears stale fields/presence and ends the game timer. With no active supported DLL, companion mode is unavailable; it does not fall back to invented values.

The wire protocol is versioned, length-prefixed, field-allowlisted, and authenticated for one user-configured session. Protocol v1 uses companion-initiated TCP, a 4-byte big-endian frame length (maximum 4096 bytes), JSON messages, a fresh 32-byte server challenge, and HMAC-SHA256 with a 32-byte key entered independently in the desktop app and companion config. MAC input uses domain separation, challenge, and a strictly increasing sequence; telemetry is signed over a canonical compact JSON encoding with fixed field order, all optional fields present (null when absent), no whitespace, no unknown or duplicate keys, and `mac` appended last on the wire. Golden vectors cover this exact encoding. Bind only to the selected private LAN interface by default. Accept telemetry records only after authentication; there are no remote commands. Reject unknown versions, unknown JSON fields, invalid tags/IDs/ranges, bad MACs, repeated sequences, and idle sessions after a configurable heartbeat timeout. Never place Discord tokens, NOVA passwords, CPU keys, NAND data, or arbitrary memory contents in a companion message. The shared key is a companion-specific secret and must use the app's secure credential storage, never normal settings or diagnostics.

The UI labels every companion field as “reported” and may show unavailable for metrics the DLL cannot obtain. Title ID and elapsed time are not inferred from a sideload config alone; they require a fresh message from a running title. Profile, achievement, gamerscore, and CPU/GPU readings stay absent until their APIs are verified on hardware.

The official Peer Pressure wiki documents sideload INI search precedence, title/media/version matching, title-DLL loading before the game entrypoint/CRT initialization, and unload when the game exits. A buildable reference exists in RB3Enhanced: XDK compiler/linker, `_DllMainCRTStartup`, and `imagexex.exe` packaging. This is a credible title-DLL build path, but not proof our output loads in Peer Pressure. The app may carry only a source skeleton until an XDK build can be run and structurally inspected; never label build success as loader compatibility.

The DLL attach callback must remain minimal. Do not start sockets, threads, or CRT-dependent work there until the Xbox 360 loader/CRT sequence and safe APIs are established for this build. The only intended initial runtime data is a heartbeat plus a Title ID obtained through a documented read-only API; the currently cited loader docs do not provide such a Title ID API. Never use a boot-resident service, DashLaunch plugin, kernel/XAM hook, memory patch/scan, or NAND/boot modification. A title transition is a disconnect and stale values are cleared.

The current `xbox-companion/` executable is a host simulation, not an Xbox binary. Its optional Title ID is manually supplied test input and the UI/protocol must not represent it as read from a console. It shares the existing key/challenge/HMAC framing by depending on the desktop crate. It sends repeated telemetry on one authenticated connection until a finite count completes or the host process is stopped.

## Artwork

Resolve game name and cover by exact Title ID through an ordered provider chain (x360db, then XboxUnity fallback). Prefer covers explicitly marked official. The live title name and cover are separate fields: if no exact metadata match exists, retain raw Title ID and use the generic Xbox 360 image. Do not guess identity from nearby IDs, text search, or image similarity. Discord official local IPC receives the provider's public HTTPS image URL in `assets.large_image`; no per-game app asset uploads are needed. The UI uses locally cached image bytes where available. Offline cache does not make a local file URL fetchable by Discord, so presence uses the generic uploaded Xbox 360 asset when no provider URL is usable. Small image remains the project icon asset placeholder.

Provider adapters return metadata, validated remote image URL, provenance, official/community status, and optionally bounded cached image bytes. Cache metadata and images with an explicit cap and TTL, throttle each provider, use short timeouts, and negative-cache not-found results briefly to avoid request storms. Do not bulk scrape providers. Do not bundle provider artwork absent redistribution rights. x360db documents broad coverage (~6,000 Xbox 360 IDs) and a per-title boxart field, but no explicit cover-art license was found. XboxUnity's documented API exposes official/community flags and cover endpoints, but reliability/hotlinking and original artwork permissions are unverified. GameTDB is not integrated until Xbox 360 coverage is confirmed.

The official Discord RPC-over-IPC documentation defines `SET_ACTIVITY` and activity assets, while Discord Rich Presence documentation allows external image URLs. This verifies protocol-level support for URLs, not the availability or fetching of a particular cover host by every Discord client. The provider URL must be HTTPS and publicly reachable; on invalid/missing provider data use the generic asset. Preserve existing local IPC-only Discord behavior.

The artwork diagnostics panel lists normalized observed Title ID, resolved title or unresolved status, provider provenance, public image URL, local cache status, and fallback reason. It must not print request headers, stored credentials, or raw provider error bodies. Live provider checks and visible Discord rendering are opt-in/manual; fixtures and IPC serialization remain separate evidence. If provider hosts cannot resolve or a desktop Discord IPC socket is absent, report those checks as blocked.

CLI workflow: `cargo run -- --artwork-diagnostics 4D5307E6` performs live provider checks for Halo 3's known Title ID and prints provider outcomes plus cover transport/cache validation; `cargo run -- --discord-test 4D5307E6` publishes only if actual provider metadata and public artwork resolve, prints an IPC acknowledgment or error, waits up to five minutes (60 seconds by default), then clears presence. `4D53085B` (Halo: Reach) and `584111F7` (Minecraft: Xbox 360 Edition) are additional known test IDs. A known ID is only an input; title, provider, and artwork are printed only if returned by a provider. The CLI's IPC acknowledgment is not a rendering result. A human must follow the manual checklist in `testing.md` and record visible success separately.
