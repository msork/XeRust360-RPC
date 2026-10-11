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
