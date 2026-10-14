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
- Large image: per-title Discord asset key only when a valid mapping exists; otherwise generic Xbox 360 placeholder key.
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

## Artwork

Use a game-specific Discord image key for the curated title catalog when the selected Discord application contains that key. The setup guide lists the six initial keys and artwork requirement. Unknown or unconfigured titles use the generic Xbox 360 placeholder; do not scrape arbitrary images at runtime or claim an image belongs to an unknown title. The small image key is the project icon placeholder.
