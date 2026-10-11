# Design

## Desktop experience

The main window is a compact dashboard with a title bar, console connection section, Rich Presence preview card, settings section, and diagnostics drawer. The UI uses a restrained dark palette with clear text contrast and Xbox green as a small accent. The preview mirrors Discord's line order and updates from either live data or mock mode.

### Main view

- Header: XeRust360-RPC name/icon, running state, and menu.
- Console: host, Discover, Connect/Disconnect, and status (Searching, Connected, Reconnecting, Unavailable).
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

## Artwork

Use a game-specific Discord image key for the curated title catalog when the selected Discord application contains that key. The setup guide lists the six initial keys and artwork requirement. Unknown or unconfigured titles use the generic Xbox 360 placeholder; do not scrape arbitrary images at runtime or claim an image belongs to an unknown title. The small image key is the project icon placeholder.
