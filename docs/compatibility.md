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

Peer Pressure's public README describes DLL sideloading into games/apps and explicitly states its implementation source is not published. Its docs exclude DashLaunch plugins and warn force-loading arbitrary DLLs may destabilize the softmod. No public loader entrypoint, import ABI, initialization/teardown contract, telemetry API, or promise of execution across title transitions was found. A persistent softmod must not be confused with persistent execution of a title DLL.

Free60 libXenon documents a PowerPC toolchain for bare-metal homebrew applications. It is not evidence of compatibility with a game-process DLL. OXDK documents Xenon Microsoft ABI and an XEX title application build/import-resolution path using XDK headers/libraries, but does not document Peer Pressure's DLL sideload contract. Neither alone passes the prototype gate. No PPC DLL/XEX artifact has been created or compiled.

The desktop protocol implementation is mock-tested. A future prototype requires evidence for the Xenon game ABI, compiler/linker/imports, package format, Peer Pressure load entrypoint, and teardown/title lifecycle. Only then may a harmless heartbeat and runtime-observed Title ID be considered. No safe title-wide API for achievements, profile, gamerscore, or temperature has been established; those values remain unavailable. Toolchain build, Peer Pressure loading, and hardware behavior remain unverified. Automatic deployment is out of scope.

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
