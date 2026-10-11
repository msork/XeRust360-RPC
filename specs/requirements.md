# Requirements

## Product

XeRust360-RPC is a native desktop companion for Linux, Windows, and macOS. It reads Xbox 360 telemetry from Aurora's NOVA LAN API and publishes it through the local Discord desktop client's official Rich Presence IPC. It does not modify the console or authenticate to Discord as a user.

## Users and hardware

- Primary target: Xbox 360 running Aurora with NOVA enabled, including consoles booted through BadUpdate/ABadAvatar.
- Secondary target: JTAG/RGH consoles running Aurora/NOVA.
- A console connection requires its LAN address, and may require NOVA credentials. Discovery and reconnect are best-effort conveniences.
- The app must clearly report when Aurora/NOVA, a metric, Discord, or game artwork is unavailable. It must never invent telemetry.

## Functional requirements

1. Configure NOVA host, port (default 9999), optional username/password, polling interval, and reconnect behavior.
2. Discover NOVA consoles on the local network where practical. Manual host entry always remains available.
3. Poll NOVA for running title and Title ID, signed-in gamertag, achievement progress, game gamerscore and total gamerscore, and CPU/GPU temperatures. Parse only values actually returned. Missing values stay missing.
4. Reconnect after transient network/API failures with bounded backoff and expose connection state and useful diagnostics.
5. Resolve a game title from a bundled or updateable Title ID catalog when possible; otherwise show the raw Title ID and label the name unresolved.
6. Publish Rich Presence only through Discord's local RPC/IPC interface. Detect Discord availability and retry IPC connection without user tokens, gateway sessions, self-bots, or direct web API login.
7. Presence uses application name “Xbox 360”; Details is the game name; State formats gamertag, X/Y achievements, game GS, total GS, and CPU/GPU °C. Independent settings control each State component. Omit unavailable components instead of substituting zero or guessed values. Include elapsed time since the current game was first observed.
8. Set large artwork to a known Discord application asset for the game when available. Until a catalog/asset mapping exists, show a neutral placeholder. Small artwork uses a project icon asset. Missing artwork must not break presence.
9. Provide a minimal native GUI with live presence preview, connection/Discord status, independent stat toggles, secure credential storage, settings persistence, and diagnostics that avoid exposing credentials.
10. Provide tray controls for show/hide, pausing presence, and quitting on supported platforms. Current implementation supports Linux; Windows and macOS tray support is outstanding.
10. Include a mock telemetry source and automated tests for parsing, formatting, reconnect state, and presence serialization. Add CI for supported host builds.

## Security and privacy

- Discord credentials/tokens are prohibited. Discord IPC uses the user's running desktop client.
- NOVA credentials are optional and stored through OS keychain facilities where available; otherwise explain the fallback and avoid plaintext diagnostics/logs.
- Telemetry stays on the local network except the local Discord IPC connection. No telemetry or credentials are sent to a project server.
- Discovery is limited to the local network and must not scan arbitrary public address ranges.

## Acceptance criteria

- A first-run user can run the app with mock telemetry on all three desktop OS targets without an Xbox.
- A configured NOVA host can be queried, and missing endpoints/authentication errors are visible without crashing.
- No test or source path implements Discord user-token authentication.
- Presence fields remain within Discord RPC limits; oversized text is truncated at UTF-8 boundaries.
- Game changes reset the elapsed timer; transient disconnects do not fabricate a game or telemetry values.
- The app can be built and checked on Linux; CI declares Linux, Windows, and macOS jobs.

## Research basis and constraints

- XeCord demonstrates the console-plugin approach but its README explicitly says it uses Discord's User Gateway and asks users to provide a user token. That approach is excluded.
- NOVA 0.7b.2 r1622 documentation describes Aurora-hosted API at port 9999 and endpoints including `/title`, `/profile`, `/achievement`, `/achievement/player`, `/temperature`, and `/authenticate`.
- The supplied Reddit beta-preview URL could not be retrieved during research; no requirements are inferred from its contents.
- Nova's public documentation does not guarantee that every requested stat is returned by every dashboard/plugin version. Parsers and UI must represent unavailable data honestly.
- Discord's official Rich Presence documentation describes `name`, `details`, `state`, `timestamps`, and `assets`; official SDK field documentation caps detail text at 128 characters. The app enforces a stricter 128 UTF-8-byte cap. The integration uses the local RPC/IPC protocol.
