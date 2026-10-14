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
11. Preserve Aurora NOVA as the supported live telemetry path for compatible BadUpdate/ABadAvatar and JTAG/RGH setups.
12. Add an optional read-only LAN companion protocol for a user-provided Peer Pressure per-title DLL. The desktop listener accepts only documented, allowlisted telemetry messages and does not install, configure, or deploy console files.
13. Treat a Peer Pressure DLL as title-scoped and potentially absent: it is loaded only for games explicitly configured for sideloading, and its lifetime ends with the host title/process. Never assume it runs at dashboard boot or across game transitions.
14. Never claim a metric the companion cannot observe from a stable, documented source. Title ID and title lifecycle are candidate observations; gamertag, achievements, gamerscore, and CPU/GPU temperatures remain unavailable unless a supported title/runtime API is demonstrated. Never inspect or patch arbitrary game memory.
15. Include mock companion telemetry, protocol security/compatibility tests, a console compatibility matrix, and cross-platform desktop CI. Real console hardware validation remains explicitly marked unverified until performed.
16. Discord presence remains exclusively official local RPC/SDK. No Discord user tokens, self-bots, Gateway automation, or credential extraction.

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
- Peer Pressure's public README/wiki documents per-title DLL sideloading, not a general telemetry API. Its source is not published. Sideload execution is game-scoped and must not be treated as persistent. It excludes DashLaunch/plugins; force-loaded DLLs may destabilize the softmod.
- The standard libXenon toolchain targets bare-metal homebrew and does not produce a Peer Pressure-compatible game DLL by itself. A game DLL requires a Xenon/XDK-compatible ABI, imports and XEX/DLL packaging; toolchain and runtime compatibility are not verified on hardware.
- OXDK documents a Xenon Microsoft ABI and XEX title-app build/import-resolution flow, but does not establish Peer Pressure's per-title DLL loader ABI, imports, initialization/teardown contract, or title transition behavior. An XEX-capable toolchain alone does not justify a DLL prototype.
- Peer Pressure is a persistent softmod, but public docs describe DLL sideloading into games/apps and do not promise that a game DLL survives title exit or dashboard transitions. Treat these as session disconnects.
- No documented safe title-wide API was established for runtime Title ID, signed-in profile, achievements, gamerscore, or temperatures from a sideloaded DLL. These remain unavailable until documented and hardware-validated.
- Peer Pressure's Softmod Features wiki documents sideload INI lookup, matching by title/media/version, loading the title DLL after the game XEX is mapped but before the game entrypoint/CRT initialization, and unloading that title DLL when the game exits. This establishes an observable process-lifecycle boundary, not hardware validation for this project.
- The official wiki calls for a title DLL, not a system DLL. A real-world title DLL build (RB3Enhanced) documents XDK `cl.exe`/`link.exe` plus `imagexex.exe`, `_DllMainCRTStartup`, and optional Wine invocation on Linux/macOS. Its build does not verify Peer Pressure loading. No XDK toolchain is installed in this environment.
- TCP integration tests must report an explicit SKIPPED reason if the host denies localhost sockets. CI must set a require-loopback flag and run all three socket scenarios on Linux, Windows, and macOS; inability to bind must fail CI.
- The requested Reddit URL remains unavailable, so it provides no confirmed technical contract.
- Research links: [Peer Pressure README](https://github.com/grimdoomer/Xbox360PeerPressure/blob/main/README.md), [Peer Pressure Softmod Features](https://github.com/grimdoomer/Xbox360PeerPressure/wiki/Softmod-Features), [Peer Pressure compatibility summary](https://consolemods.org/wiki/Xbox_360:Peer_Pressure), [Peer Pressure issue discussion](https://github.com/grimdoomer/Xbox360PeerPressure/issues/1), [XeCord README](https://github.com/UncreativeXenon/XeCord/blob/master/README.md), [libXenon toolchain](https://github.com/Free60Project/libxenon), [Xenon ABI/XEX toolchain notes](https://github.com/MrMilenko/OXDK/blob/main/docs/xbox360.md), and the [requested Reddit preview](https://www.reddit.com/r/360hacks/comments/1vma14r/discord_presence_beta_preview/) (unavailable during research).
- Nova's public documentation does not guarantee that every requested stat is returned by every dashboard/plugin version. Parsers and UI must represent unavailable data honestly.
- Discord's official Rich Presence documentation describes `name`, `details`, `state`, `timestamps`, and `assets`; official SDK field documentation caps detail text at 128 characters. The app enforces a stricter 128 UTF-8-byte cap. The integration uses the local RPC/IPC protocol.
