# Testing strategy

## Principles

- Test pure domain and formatting logic without network, Discord, or Xbox hardware.
- Use recorded NOVA response fixtures; never require real credentials in CI.
- Use mock telemetry and an IPC test server to exercise orchestration.
- Keep integration failures diagnosable, and never log credential material.
- The test matrix is defined in `tasks.md`; this file defines test layers and commands.

## Layers

1. Unit tests: response decoding, unavailable fields, presence composition, UTF-8 truncation, timer transitions, retry backoff, settings serialization/redaction.
2. Fixture integration tests: schema-shaped NOVA `/title`, `/profile`, achievement IDs/`cred`/profile slots, `/temperature` Celsius/Fahrenheit units, auth token redaction, partial/malformed responses, HTTP chunking/204, and HTTP timeout.
3. IPC protocol tests: in-memory framed Discord handshake, activity update payload, clear command, invalid application IDs, and invalid payload bounds. Manual checks verify real Unix socket/named pipe discovery against a running client.
4. GUI smoke checks: mock mode, toggle behavior, diagnostics, and settings round trip; Linux tray actions; manual checks where native desktop automation is unavailable. Windows/macOS tray coverage is pending their tray implementation.
5. Build/CI checks: formatting, tests, clippy, and compile jobs for Linux, Windows, and macOS.
6. Companion protocol tests: local loopback mock peer; authenticated connection and telemetry, orderly TCP EOF, authenticated malformed/invalid frames, payload bounds, HMAC challenge/golden vectors, versioning, fixed serialization, monotonic sequence/freshness, idle heartbeat timeout, explicit process/session termination, and omitted metric preservation. EOF must be distinguished from a read timeout so intentional title-process exit clears immediately. No console required.
7. Compatibility evidence: maintain [`../docs/compatibility.md`](../docs/compatibility.md) by boot method, motherboard class, kernel, dashboard/source, and DLL lifecycle. A cell is “verified” only with real hardware evidence; otherwise label it “not tested” or “not applicable”.
8. Xbox artifact gate: before adding a PPC binary, record the compiler/ABI, imports/linking, packager, Peer Pressure load entrypoint, and teardown/lifecycle evidence. An OXDK title XEX build or libXenon bare-metal build alone does not pass this gate.

## Local commands

```sh
cargo fmt --all -- --check
cargo test --workspace --offline
cargo clippy --workspace --all-targets -- -D warnings
cargo check --workspace
```

## Hardware verification (manual, optional for CI)

- Confirm Aurora/NOVA API reachable at configured host and port.
- Confirm login and achievement values match the console dashboard for a signed-in profile.
- Confirm CPU/GPU readings and units against NOVA response.
- Start/stop a title and confirm timer resets, unknown-title fallback, disconnect handling, and reconnect.
- Confirm Discord activity appears only while local Discord accepts RPC and clears on app shutdown.
- Peer Pressure: hardware validation is blocked by the missing public loader ABI and telemetry API. If evidence establishes these, verify configured game DLL load/unload per title, runtime-observed Title ID, and network loss without stale presence. Hardware work must not modify boot files through this application.
- Validate each non-title metric independently against a documented runtime API and console UI; unavailable API means unavailable telemetry.

## Compatibility matrix policy

Track Aurora NOVA on BadUpdate/ABadAvatar, Aurora NOVA on JTAG/RGH, Peer Pressure on supported Xenon/Zephyr/Falcon/Opus/Jasper/Trinity revisions, and unsupported Corona/Winchester/Waitsburg/Stingray/Barracuda revisions separately. Peer Pressure's documented motherboard support is not equivalent to this app's companion support. The companion remains unverified on all consoles until its DLL and protocol are exercised on hardware. Record title ID, kernel (currently Peer Pressure docs cite 2.0.17599), dashboard, DLL build/toolchain and observed fields for every actual test.
