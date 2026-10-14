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
6. Companion protocol tests: local loopback mock peer; authenticated connection and telemetry, orderly TCP EOF, authenticated malformed/invalid frames, payload bounds, HMAC challenge/golden vectors, versioning, fixed serialization, monotonic sequence/freshness, idle heartbeat timeout, explicit process/session termination, and omitted metric preservation. EOF must be distinguished from a read timeout so intentional title-process exit clears immediately. If loopback is denied locally, emit `SKIPPED: <reason>`; set `XERUST_REQUIRE_LOOPBACK=1` in CI so unavailable sockets fail instead of skip.
7. Compatibility evidence: maintain [`../docs/compatibility.md`](../docs/compatibility.md) by boot method, motherboard class, kernel, dashboard/source, and DLL lifecycle. A cell is “verified” only with real hardware evidence; otherwise label it “not tested” or “not applicable”.
8. Xbox artifact gate: before adding a PPC binary, record the compiler/ABI, imports/linking, packager, Peer Pressure load entrypoint, and teardown/lifecycle evidence. An OXDK or xsdk title XEX build or libXenon bare-metal build alone does not pass this gate. A successful XDK DLL build is artifact evidence only; loader compatibility requires a separately verified Peer Pressure load.
   - Treat `XamGetCurrentTitleId`/ordinal `0x1CF` as reverse-engineered only until a supported compatibility reference and hardware matrix exist; do not use it as verified telemetry.
9. Cross-platform TCP CI: Linux, Windows, and macOS jobs each run the three named localhost tests with `XERUST_REQUIRE_LOOPBACK=1` and `--nocapture`. A green result means the tests actually bound and used loopback sockets, not that they returned early.
10. Host companion simulation: compile `xbox-companion/` on Linux, Windows, and macOS. A loopback integration test must show one authenticated connection carrying multiple monotonically increasing heartbeat records and ending cleanly; a supplied candidate Title ID is the only populated telemetry field and is not described as console-observed.
11. Artwork providers: inject mock responses/time and verify exact IDs, known IDs (`4D5307E6`, `4D53085B`, `584111F7`), unknown IDs, alternate/multi-disc IDs, official-cover preference, source fallback, URL validation, request throttling, bounded response handling, metadata/image cache hits and expiry, offline cache, and provider errors. Keep live network tests opt-in and one-title only.
12. Discord cover payload: assert local IPC `SET_ACTIVITY` serializes an HTTPS cover URL to `assets.large_image`, and the generic asset when unavailable. This is a protocol serialization check; actual Discord-client fetching/rendering must remain reported as unverified until a manual live-client check succeeds.
13. Provider transport: test HTTPS-only validation, no private/loopback image fetch targets, bounded response size/time arguments, and graceful no-curl/provider-DNS failure without interrupting console telemetry. Verify provider requests contain only a Title ID and no NOVA credentials or profile fields.
14. Artwork diagnostics: verify Title ID, resolved/unresolved title, provenance, public image URL, cache hit/miss, and fallback reason for success, offline cache, unknown ID, missing cover, invalid URL, and provider outage. Assert credentials and raw response bodies are absent.
15. Live provider smoke checks (opt-in, one ID per request): capture HTTP status, redirect count/final HTTPS URL, content type, bounded body size, exact-ID metadata match, image signature, cache repeat behavior, throttling, and outage behavior for x360db and XboxUnity. DNS/network denial is recorded as blocked, never passed or interpreted as provider failure.
16. Discord image rendering: serialize external HTTPS URL into official local RPC `SET_ACTIVITY`, publish with a running desktop client, and visually confirm the Rich Presence card renders the image. Record client/platform, URL, and visible result. IPC acknowledgment alone is not a pass; no client means blocked.
17. Coverage classes: test a disc title, XBLA title, obscure title, and Original Xbox ID only where the provider explicitly identifies platform. Record whether Title ID/media ID distinguishes multi-disc variants; never infer the disc from Title ID alone.

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
- Peer Pressure: official docs establish title-DLL configuration and unload-on-game-exit. Build a skeleton only with an available XDK toolchain; distinguish compile/package checks from actual Peer Pressure loading. No app-assisted deployment or hardware modification.
- Host simulation: `cargo run --manifest-path xbox-companion/Cargo.toml -- --help` is host-only and does not prove Xenon ABI or title DLL compatibility.
- Validate each non-title metric independently against a documented runtime API and console UI; unavailable API means unavailable telemetry.

## Compatibility matrix policy

Track Aurora NOVA on BadUpdate/ABadAvatar, Aurora NOVA on JTAG/RGH, Peer Pressure on supported Xenon/Zephyr/Falcon/Opus/Jasper/Trinity revisions, and unsupported Corona/Winchester/Waitsburg/Stingray/Barracuda revisions separately. Peer Pressure's documented motherboard support is not equivalent to this app's companion support. The companion remains unverified on all consoles until its DLL and protocol are exercised on hardware. Record title ID, kernel (currently Peer Pressure docs cite 2.0.17599), dashboard, DLL build/toolchain and observed fields for every actual test.
