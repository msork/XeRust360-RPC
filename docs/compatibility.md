# Compatibility and evidence matrix

“Documented” describes the upstream project only. It does not mean XeRust360-RPC or an Xbox companion DLL has been verified on that setup. No Xbox hardware tests have been run for this project yet.

## Boot method and console compatibility

| Console setup | Upstream evidence | XeRust360-RPC Aurora/NOVA path | Peer Pressure companion path |
|---|---|---|---|
| BadUpdate / ABadAvatar with Aurora + NOVA | Existing app targets NOVA's LAN API. Actual console/API combinations are not hardware-verified here. | Expected to work where the NOVA API is reachable; unsupported fields stay missing. | Not applicable unless Peer Pressure is separately installed and a per-title DLL is configured. |
| JTAG / RGH with Aurora + NOVA | Existing app targets NOVA's LAN API. Actual console/API combinations are not hardware-verified here. | Expected to work where the NOVA API is reachable; unsupported fields stay missing. | Not applicable unless Peer Pressure is separately installed and a per-title DLL is configured. |
| Peer Pressure on Xenon, Zephyr, Falcon/Opus, Jasper/Tonasket, Trinity | Peer Pressure documentation lists supported revisions at kernel 2.0.17599, subject to its motherboard caveats. | Aurora/NOVA on this setup has not been verified. | This app's DLL and telemetry path are unverified. Upstream describes DLL sideloading on selected game launches, not a general telemetry service. |
| Peer Pressure on Corona, Winchester, Waitsburg, Stingray, or Barracuda | Upstream compatibility summary marks these hardware families unsupported for Peer Pressure. | NOVA compatibility depends on a separately working Aurora/NOVA setup; not verified here. | Not supported by the documented Peer Pressure base softmod; companion is not applicable. |
| Peer Pressure on any console with no title-specific DLL configured | Per-title DLL sideload configuration is required. | Independent NOVA path may still work if available. | No companion process is assumed. Presence must clear when no authenticated telemetry arrives. |
| Dashboard, title transition, or game exit | A title-scoped DLL cannot be assumed to remain loaded across process lifecycle changes. | NOVA may continue independently if available. | Session is scoped to the authenticated connection; disconnect/heartbeat expiry clears the game and timer. |

The upstream Peer Pressure motherboard list is reproduced as upstream documentation, not a recommendation or a claim that this application works on those revisions. Peer Pressure's documentation also excludes DashLaunch and its plugins and warns that force-loading arbitrary DLLs may destabilize its softmod. This project does not install or alter any console files.

## Companion telemetry availability

| Field | Current companion status | Evidence needed before enabling from a real DLL |
|---|---|---|
| Active Title ID | Protocol supports a reported Title ID; no Xbox DLL currently sends it. | A documented title/runtime API or safe supported source, verified only while a configured title is running. |
| Game name/art | Desktop may resolve a reported Title ID through its catalog; unknown IDs remain unresolved. | Accurate Title ID observation; artwork still depends on configured Discord assets. |
| Gamertag | Unavailable | Stable, documented signed-in profile API and hardware validation. |
| Achievements X/Y | Unavailable | Title-independent achievement totals/progress API with correct profile-slot semantics and hardware validation. |
| Game / total gamerscore | Unavailable | Documented API returning verified values and hardware comparison. |
| CPU/GPU temperature | Unavailable | Documented sensor API, declared unit, and hardware comparison. |
| Elapsed game time | Desktop measures from the first fresh Title ID observation per authenticated session. | A valid companion session; timer resets on title change, disconnect, or expiry. |

No memory scanning, arbitrary title-memory reads, patching, kernel hooks, or guessed values may be used to fill these gaps.

## Toolchain and lifecycle status

### Evidence-backed loader/toolchain matrix

| Evidence path | Build target and entrypoint/imports | Peer Pressure fit | Evidence status |
|---|---|---|---|
| Peer Pressure official Softmod Features wiki | Looks for `<game xex directory>\\sideload.ini`, then `<game xex directory>\\<title id>.ini`, then `Hdd1:\\PeerPressure\\SideLoading\\<title id>.ini`. Matching config supports title ID, media ID, version. Loads after game XEX mapping and before game entrypoint/CRT initialization. Requires a title DLL (not system DLL); unloads at game exit; says not to hook outside game XEX. | Explicitly documented loader behavior and process boundary. Does not publish loader source, an import/ordinal table, a telemetry API, or an example companion DLL. | Documented upstream; not tested with this project. |
| Microsoft Xbox 360 XDK (used by RB3Enhanced) | `cl.exe` and `link.exe` build a DLL with `_DllMainCRTStartup`; `imagexex.exe` wraps the result as Xbox DLL/XEX. RB3Enhanced links `xapilib`, `xboxkrnl`, `xnet`, and `xonline` in its project configuration. Its build guide supports Windows and Wine on Linux/macOS when the user supplies XDK. | Credible build path for title DLLs. RB3Enhanced is a real game mod, but uses its own loader/plugin paths; its build is not proof of Peer Pressure acceptance. The imports list is evidence of available libraries, not a safe networking recipe for this companion. | Documented by upstream mod/build scripts; XeRust360-RPC has not built it. XDK is absent here. |
| OXDK (patched LLVM + supplied XDK libraries) | Custom Xenon ABI compiler, linker/import resolver and XEX2 wrapper; documented sample is an XEX title application. Public docs do not demonstrate a title-DLL output recipe/entrypoint. | Useful ABI/toolchain research; insufficient by itself for a Peer Pressure title DLL. | Documented XEX title flow; DLL compatibility unverified. |
| xenosworks/xsdk (open-source) | GCC 15 `powerpc-xsdk-elf` plus newlib; builds ELF into XEX2 title applications using ordinal imports. Its README reports hardware-tested title SDK milestones and title-based tests. No DLL module recipe or Peer Pressure DLL behavior is documented. | Potential open-source title-app toolchain; not currently a title-DLL path. | Project reports hardware tests for its title SDK, not for DLLs or Peer Pressure. |
| Free60 libXenon | PowerPC bare-metal homebrew toolchain. | Not a game-process title DLL toolchain. | Documented target distinction. |

The official loader docs plus RB3Enhanced's XDK DLL project establish a credible build path in principle, but not enough to claim a built DLL works in Peer Pressure. A structurally validated XDK DLL build and actual Peer Pressure load test are separate gates. The XDK is not installed in this environment, and no DLL binary has been produced. RB3Enhanced's `/ENTRY:"_DllMainCRTStartup"` shows a conventional CRT startup wrapper and its `imagexex.exe` invocation shows XEX-DLL packaging. However, Peer Pressure calls the DLL before the game entrypoint and the game CRT initialization; whether the DLL's own CRT wrapper fully initializes the DLL CRT before its callback must be verified from the XDK artifact/toolchain and a safe test environment. Microsoft Windows DLL guidance recommends minimal work in DllMain and deferred initialization; we apply that conservatively, but it is not direct proof of Xbox 360 loader internals. Do not perform network initialization, blocking work, or thread shutdown in DLL entrypoints until that lifecycle is verified. XDK project references `xnet.lib`, but no validated safe init/connect/cleanup sequence for this sideload use was found. No documented read-only Title ID API has yet been established. Gamertag, achievements, gamerscore and temperatures remain unavailable.

`xbox-companion/` therefore provides a host-side protocol simulator only. It can compile and send authenticated heartbeats over the existing desktop protocol. Any candidate Title ID passed on its command line is simulation data; it is not retrieved from an Xbox and does not prove the XDK DLL or Peer Pressure path.

`XamGetCurrentTitleId` appears at XAM ordinal `0x1CF` in reverse-engineered export tables and public game-mod code, including examples that also perform game hooks. This establishes that the export is known in community reverse engineering; it does not establish a supported/public Xbox 360 API contract, compatibility across Peer Pressure's kernel versions, or safe use from this DLL lifecycle. XeRust360-RPC does not call it. The title ID field therefore remains unavailable from a real companion until a read-only API is documented and hardware-validated.

Peer Pressure's public README says source is not published, excludes DashLaunch plugins, and warns force-loading arbitrary DLLs may destabilize the softmod. A persistent softmod is not persistent title DLL execution. No deployment or console modification is within this project.

## References

- [Peer Pressure README](https://github.com/grimdoomer/Xbox360PeerPressure/blob/main/README.md)
- [Peer Pressure Softmod Features](https://github.com/grimdoomer/Xbox360PeerPressure/wiki/Softmod-Features)
- [Peer Pressure hardware compatibility summary](https://consolemods.org/wiki/Xbox_360:Peer_Pressure)
- [Peer Pressure project discussion](https://github.com/grimdoomer/Xbox360PeerPressure/issues/1)
- [XeCord README](https://github.com/UncreativeXenon/XeCord/blob/master/README.md)
- [libXenon](https://github.com/Free60Project/libxenon)
- [Xenon ABI/XEX toolchain notes](https://github.com/MrMilenko/OXDK/blob/main/docs/xbox360.md)
- [Peer Pressure README (sideloading, source status, plugin limits)](https://github.com/grimdoomer/Xbox360PeerPressure)
- [Peer Pressure installation lifecycle](https://github.com/grimdoomer/Xbox360PeerPressure/wiki/Installation)
- [Peer Pressure telemetry API discussion](https://github.com/grimdoomer/Xbox360PeerPressure/issues/1)
- [BadUpdate exploit details](https://github.com/grimdoomer/Xbox360BadUpdate/wiki/Exploit-Details)
- [Peer Pressure official sideload configuration and lifecycle](https://github.com/grimdoomer/Xbox360PeerPressure/wiki/Softmod-Features)
- [RB3Enhanced real-world Xbox DLL build guide](https://github.com/RBEnhanced/RB3Enhanced/blob/master/BUILDING.md)
- [RB3Enhanced XDK DLL Makefile](https://github.com/RBEnhanced/RB3Enhanced/blob/master/Makefile)
- [Microsoft DLL entrypoint guidance](https://learn.microsoft.com/en-us/windows/win32/dlls/dllmain)
- [Microsoft DLL initialization best practices](https://learn.microsoft.com/en-us/windows/win32/dlls/dynamic-link-library-best-practices)
- [Xbox 360 XEX/DLL development guide](https://github.com/ClementDreptin/ModdingResources/blob/main/GettingStarted/getting-started.md)
- [xenosworks/xsdk open-source title XEX toolchain](https://github.com/xenosworks/xsdk)
- [Reverse-engineered XAM ordinal table](https://github.com/emoose/xbox-reversing/blob/master/x360_imports.py)
- [Example game mod using that XAM export (not used as safety evidence)](https://github.com/rheard/The-Tesseract-on-Xbox-360-for-Black-Ops-2/blob/master/main.cpp)

## Game metadata and artwork providers

| Provider | Documented coverage/API | Licensing and hotlink status | Project status |
|---|---|---|---|
| x360db (`xenia-manager/x360db`) | Public `games.json` index documents about 6,000 entries, 8-character Title IDs, related/alternate IDs, titles, media IDs, and boxart URLs. Its guide also documents per-title `info.json` and artwork paths. | Usage guide has no explicit cover-art redistribution license. URL hotlink terms are not stated. | Primary lookup adapter uses the documented index schema; no cover art is bundled. Live endpoint availability could not be checked here because outbound DNS resolution failed. The current Nova/companion runtime model does not supply media IDs. |
| XboxUnity | XboxUnity-Scraper's unofficial API reference documents `CoverInfo.php` by Title ID, an `Official` flag, and `Cover.php` by cover ID. | Scraper archive's CC BY-NC license excludes original XboxUnity data; original image rights remain with their owners. API uptime and hotlink permission are not established. | Runtime fallback adapter prefers the API's explicit official flag; it is not called verified/live until an external request succeeds. No data dump or cover files are bundled. |
| GameTDB | Public examples cover Nintendo systems; no Xbox 360 Title ID index/API was verified. | Not assessed for Xbox 360 usage. | Not integrated. |

Discord's official RPC-over-IPC documentation defines the local `SET_ACTIVITY` command and activity asset fields. Official Rich Presence SDK documentation allows a URL for large/small images. The client sends a valid public HTTPS image URL directly to Discord; the app does not proxy it. This verifies protocol support, not live Discord rendering from any specific provider. The client falls back to the uploaded generic Xbox 360 asset. Local art cache is used by the app preview and does not provide a remotely fetchable URL.

Runtime artwork requests disclose the observed Title ID to these providers only. No NOVA credentials, gamertag, achievements, gamerscore, temperatures, or other telemetry are sent. Provider requests are HTTPS-only, size/time bounded, rate-limited, cached, and may fail without stopping telemetry. The system `curl` executable is required for network lookup; absence produces provider unavailability and uses cache or fallback.

See [artwork validation results](artwork-validation.md) for the exact endpoint probes and the current blocked live/client checks. Fixture-backed resolution is not evidence of an external provider match.
