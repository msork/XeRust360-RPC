# XeRust360-RPC

A Rust desktop companion that reads Xbox 360 telemetry from Aurora's NOVA LAN API and publishes it through the local Discord desktop client's Rich Presence IPC.

## Current support

- Linux, Windows, and macOS desktop builds (CI checks all three).
- Linux StatusNotifier tray controls. Windows and macOS currently have a window only; closing the window exits the app.
- NOVA on Aurora (default port 9999), optional credentials via the OS credential store, local subnet discovery, and reconnect backoff.
- Experimental authenticated LAN listener for a user-built, title-scoped Peer Pressure companion. The app ships protocol code and a mock peer only; it does not ship, build, configure, or deploy an Xbox DLL.
- Discord Rich Presence through local IPC only. No Discord user tokens, self-bots, Gateway sessions, or web login.
- Mock telemetry mode for preview without a console or Discord.

BadUpdate/ABadAvatar and JTAG/RGH are boot methods; telemetry support depends on Aurora and NOVA running on the console. XeRust360-RPC does not install or interact with a boot exploit.

## Run

Install Rust, then run:

```sh
cargo run
```

The first run starts in mock mode. `cargo run -- --mock-preview` prints a text preview. Select the NOVA host and port in the desktop app, optionally enter credentials, then connect. NOVA discovery scans private local IPv4 networks up to /24; use manual IP entry if your LAN uses a larger subnet or discovery is blocked.

On Linux, `secret-tool` (from libsecret) stores NOVA passwords. On macOS and Windows, credentials use Keychain and Credential Manager. If a credential backend cannot be used, the password is kept in memory for that app session and is never written to settings JSON.

## Discord application setup

Create a Discord application named **Xbox 360** and use its Application ID in the app. Upload only the generic fallback and project icon assets using these case-sensitive keys:

- `xbox-360-placeholder` (generic cover fallback)
- `xerust360-rpc` (small project icon; replace with the final project icon when supplied)

Known Xbox 360 Title IDs resolve game names and box art through the x360db metadata index, with XboxUnity as a runtime artwork fallback. The cover image URL is passed directly through official Discord Rich Presence local IPC, so each game does not need an uploaded Discord asset. The local cache keeps metadata and downloaded art for the desktop preview when offline. Discord itself can only fetch a public provider URL; when one is unavailable the presence uses the generic asset. Unknown Title IDs stay unresolved; no title is guessed.

The metadata providers are community resources. x360db documents about 6,000 Xbox 360 entries, alternate IDs, media IDs, and per-title box-art URLs, but does not document a separate cover-art license. XboxUnity's cover API documents an official/community flag; its archive license excludes original cover images and does not establish permission for those images, API uptime, or hotlinking. XeRust360-RPC does not bundle or rehost cover art. Provider traffic includes the currently observed Title ID; no credentials or other telemetry are sent to these providers. You can keep the app offline to use locally cached metadata/artwork, or rely on the generic Discord asset. Title ID-only telemetry cannot select between discs sharing a Title ID. Original Xbox identification is not claimed unless a provider explicitly identifies the platform. Artwork diagnostics show title ID, match/provider, image URL, cache state, and fallback reason.

Artwork lookup uses the system `curl` executable for bounded HTTPS requests (connect/total timeout and response-size caps). It is available by default on current Windows, macOS, and common Linux installations; if missing or blocked, title telemetry continues and a provider diagnostic is shown.

Discord must be running as the current desktop user for local IPC presence to appear. The app shows its preview even when Discord is unavailable.

## NOVA data and limitations

The app reads title ID, signed-in profile/gamertag/gamerscore, achievement progress, and CPU/GPU temperatures from NOVA endpoints. NOVA versions or dashboard plugins may omit some fields. Unavailable values are omitted from presence; they are never replaced with invented values. NOVA's achievement data is indexed by signed-in profile slot. Temperatures are converted to Celsius when NOVA declares Fahrenheit.

All console traffic remains on the local network. Settings contain no password. Diagnostics redact credentials and tokens. For implementation details, acceptance criteria, and test cases, see [`specs/`](specs/).

### Peer Pressure companion (experimental)

Peer Pressure documents DLL sideloading for specific title launches. A sideloaded DLL is not a console-wide daemon: dashboard use, unsupported titles, title transitions, and process exit can leave no companion running. XeRust360-RPC's optional listener accepts an authenticated, read-only protocol from a configured companion and clears the game session when its connection expires. Protocol v1 uses length-prefixed JSON and HMAC-SHA256 challenge authentication; the session key belongs in the OS credential store and must also be configured in the separately developed DLL.

The listener can be exercised with the included mock sender. **No Xbox companion DLL is included or hardware-verified.** No stable public Peer Pressure telemetry API is documented. A running title companion may report its Title ID; gamertag, achievements, gamerscore, and CPU/GPU temperatures remain unavailable until a safe, documented source is proven. No memory scans/patches, kernel hooks, boot changes, NAND writes, DashLaunch plugins, or automatic deployment are used.

See [`docs/compatibility.md`](docs/compatibility.md) for the current boot-method and motherboard evidence matrix. Peer Pressure's own documented support does not imply this app's DLL companion is compatible.

## Build and verify

```sh
cargo fmt --all -- --check
cargo test --workspace --offline
cargo clippy --workspace --all-targets --offline -- -D warnings
```

Cross-platform checks run in GitHub Actions for Ubuntu, Windows, and macOS. Unit/fixture tests verify external URL serialization into Discord `SET_ACTIVITY`; this environment has no Discord IPC socket and cannot resolve x360db or XboxUnity, so live provider fetches and visible Discord rendering remain unverified. Native hardware and real Discord client checks remain manual; see [`specs/testing.md`](specs/testing.md).

## License

MIT. See [LICENSE](LICENSE).
