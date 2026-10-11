# XeRust360-RPC

A Rust desktop companion that reads Xbox 360 telemetry from Aurora's NOVA LAN API and publishes it through the local Discord desktop client's Rich Presence IPC.

## Current support

- Linux, Windows, and macOS desktop builds (CI checks all three).
- Linux StatusNotifier tray controls. Windows and macOS currently have a window only; closing the window exits the app.
- NOVA on Aurora (default port 9999), optional credentials via the OS credential store, local subnet discovery, and reconnect backoff.
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

Create a Discord application named **Xbox 360** and use its Application ID in the app. Upload artwork assets with these case-sensitive keys to enable matching images:

- `xbox360` (generic fallback and small project icon)
- `halo3`, `haloreach`, `halowars`, `halo4`, `gtav`, `skyrim`

Unknown games use `xbox360`; known game titles without uploaded assets may display no artwork. The game catalog is intentionally small and unknown Title IDs remain unresolved instead of being guessed.

Discord must be running as the current desktop user for local IPC presence to appear. The app shows its preview even when Discord is unavailable.

## NOVA data and limitations

The app reads title ID, signed-in profile/gamertag/gamerscore, achievement progress, and CPU/GPU temperatures from NOVA endpoints. NOVA versions or dashboard plugins may omit some fields. Unavailable values are omitted from presence; they are never replaced with invented values. NOVA's achievement data is indexed by signed-in profile slot. Temperatures are converted to Celsius when NOVA declares Fahrenheit.

All console traffic remains on the local network. Settings contain no password. Diagnostics redact credentials and tokens. For implementation details, acceptance criteria, and test cases, see [`specs/`](specs/).

## Build and verify

```sh
cargo fmt --all -- --check
cargo test --workspace --offline
cargo clippy --workspace --all-targets --offline -- -D warnings
```

Cross-platform checks run in GitHub Actions for Ubuntu, Windows, and macOS. Native hardware and real Discord client checks remain manual; see [`specs/testing.md`](specs/testing.md).

## License

MIT. See [LICENSE](LICENSE).
