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
| Microsoft Xbox 360 XDK (used by RB3Enhanced) | `cl.exe` and `link.exe` build a DLL with `_DllMainCRTStartup`; `imagexex.exe` wraps the result as Xbox DLL/XEX. RB3Enhanced links `xapilib`, `xboxkrnl`, `xnet`, and `xonline` in its project configuration. Its build guide supports Windows and Wine on Linux/macOS when the user supplies XDK. | Credible build path for title DLLs. RB3Enhanced is a real game mod, but uses its own loader/plugin paths; its build is not proof of Peer Pressure acceptance. | Documented by upstream mod/build scripts; XeRust360-RPC has not built it. XDK is absent here. |
| OXDK (patched LLVM + supplied XDK libraries) | Custom Xenon ABI compiler, linker/import resolver and XEX2 wrapper; documented sample is an XEX title application. Public docs do not demonstrate a title-DLL output recipe/entrypoint. | Useful ABI/toolchain research; insufficient by itself for a Peer Pressure title DLL. | Documented XEX title flow; DLL compatibility unverified. |
| xenosworks/xsdk (open-source) | GCC 15 `powerpc-xsdk-elf` plus newlib; builds ELF into XEX2 title applications using ordinal imports. Its README reports hardware-tested title SDK milestones and title-based tests. No DLL module recipe or Peer Pressure DLL behavior is documented. | Potential open-source title-app toolchain; not currently a title-DLL path. | Project reports hardware tests for its title SDK, not for DLLs or Peer Pressure. |
| Free60 libXenon | PowerPC bare-metal homebrew toolchain. | Not a game-process title DLL toolchain. | Documented target distinction. |

The official loader docs plus RB3Enhanced's XDK DLL project establish a credible build path in principle, but not enough to claim a built DLL works in Peer Pressure. A structurally validated XDK DLL build and actual Peer Pressure load test are separate gates. The XDK is not installed in this environment, and no DLL binary has been produced. DLL loading occurs before the game's CRT initialization; do not start companion sockets/threads or use CRT-dependent functions in `DllMain` without verified DLL-runtime sequencing. No documented read-only Title ID API has yet been established. Gamertag, achievements, gamerscore and temperatures remain unavailable.

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
- [Xbox 360 XEX/DLL development guide](https://github.com/ClementDreptin/ModdingResources/blob/main/GettingStarted/getting-started.md)
- [xenosworks/xsdk open-source title XEX toolchain](https://github.com/xenosworks/xsdk)
- [Reverse-engineered XAM ordinal table](https://github.com/emoose/xbox-reversing/blob/master/x360_imports.py)
- [Example game mod using that XAM export (not used as safety evidence)](https://github.com/rheard/The-Tesseract-on-Xbox-360-for-Black-Ops-2/blob/master/main.cpp)
