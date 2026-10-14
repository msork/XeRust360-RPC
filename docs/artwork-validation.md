# Artwork validation record

Date: 2026-10-13. This record separates fixture/transport tests from live provider and Discord-client checks.

## Live provider probes

The command environment could not resolve either provider host. Each request terminated at DNS lookup (`curl: (6) Could not resolve host`) before an HTTP response, so status, redirects, content type, response body, title matching, artwork, rate limiting, cache revalidation, and outage semantics were not observable from the providers.

| Probe | Request | Result | What it establishes |
|---|---|---|---|
| x360db index | `https://xenia-manager.github.io/x360db/games.json` | HTTP `000`, DNS resolution failed | No live endpoint or data match was verified. |
| XboxUnity metadata | `https://xboxunity.net/Resources/Lib/CoverInfo.php?titleid=4D5307E6` | HTTP `000`, DNS resolution failed | Halo 3's ID was sent, but no provider response/match was received. |
| XboxUnity cover route reachability probe | `https://xboxunity.net/Resources/Lib/Cover.php?cid=1&size=large` | HTTP `000`, DNS resolution failed | Reachability only was intended; `cid=1` was not established as a real game's cover ID and is not a cover test. |

The failure is an environment network/DNS blocker, not evidence that either provider is unavailable globally. No provider outage or successful artwork retrieval is claimed.

## Discord client check

No Discord desktop process or local `discord-ipc-*` socket was present. Therefore the project did not send a live `SET_ACTIVITY`, and there is no visual confirmation that Discord fetched or rendered external artwork. Official documentation supports external image URLs in Rich Presence asset fields, but actual client rendering remains blocked pending a running Discord desktop client and publicly reachable HTTPS artwork.

## Fixture and controlled transport checks

The offline Rust suite passed 47 tests, including exact Title ID matching for Halo 3 (`4D5307E6`), Halo: Reach (`4D53085B`), and Minecraft: Xbox 360 Edition (`584111F7`), plus unknown-ID behavior, alternate IDs, official-cover preference, provider fallback/failure, cache reuse/offline cache, and request interval behavior. These are fixture-backed cases and do not assert current live provider contents.

Transport unit tests validate parsing the body separately from curl response metadata, redirects/final URL fields, supported JSON MIME values, image MIME and signature, and malformed image rejection. The live redirect chain, live provider MIME headers, rate limiting at providers, and online-cache refresh remain unverified because DNS prevented HTTP requests.

## Coverage limits

| Category | Verified result |
|---|---|
| Disc games | Known Halo fixtures resolve in tests; current provider match/artwork is unverified live. |
| XBLA | Minecraft fixture resolves in tests; current provider match/artwork is unverified live. |
| Obscure games | No live exact Title ID/provider record was obtained; no result claimed. |
| Multi-disc games | x360db documents `media_id` values and per-media edition metadata, but current runtime telemetry supplies only Title ID. Disc-specific artwork cannot be selected safely when discs share a Title ID. |
| Original Xbox | Not validated or claimed. The active catalog is Xbox 360-specific; an ID is not treated as Original Xbox without an explicit provider platform label. |

To complete the blocked checks, rerun the provider probes from a network that can resolve and reach both HTTPS hosts, then use a desktop session with Discord running and observe the activity card. Record the actual final URL, status, MIME, exact returned ID/title, cache state, and visible rendering here; do not promote fixture evidence to live verification.
