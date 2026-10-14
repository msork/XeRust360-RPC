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

The Peer Pressure repository publishes documentation and binaries but no implementation source or telemetry API. Public docs describe title-scoped DLL sideloading and configuration by title/media/version. The desktop app's protocol implementation is mock-tested only. A future DLL build requires a proven Xenon PowerPC game ABI, compatible imports, and XEX/DLL packaging; bare-metal libXenon alone is not sufficient evidence. Toolchain build, Peer Pressure loading, and real-hardware behavior remain unverified. Automatic deployment is out of scope.

## References

- [Peer Pressure README](https://github.com/grimdoomer/Xbox360PeerPressure/blob/main/README.md)
- [Peer Pressure Softmod Features](https://github.com/grimdoomer/Xbox360PeerPressure/wiki/Softmod-Features)
- [Peer Pressure hardware compatibility summary](https://consolemods.org/wiki/Xbox_360:Peer_Pressure)
- [Peer Pressure project discussion](https://github.com/grimdoomer/Xbox360PeerPressure/issues/1)
- [XeCord README](https://github.com/UncreativeXenon/XeCord/blob/master/README.md)
- [libXenon](https://github.com/Free60Project/libxenon)
- [Xenon ABI/XEX toolchain notes](https://github.com/MrMilenko/OXDK/blob/main/docs/xbox360.md)
