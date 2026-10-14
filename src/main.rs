use std::{process::ExitCode, time::Duration};

use xerust360_rpc::{
    artwork::{self, ArtworkLiveCheck},
    discord::DiscordRpcClient,
    domain::{GameIdentity, StateToggles, TelemetrySnapshot},
    mock::sample_snapshot,
    presence::format_presence,
    settings,
};

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Gui,
    Help,
    MockPreview,
    ArtworkDiagnostics { title_id: String },
    DiscordTest { title_id: String, duration: u64 },
}

fn main() -> ExitCode {
    match parse_args(std::env::args().skip(1).collect()) {
        Ok(command) => ExitCode::from(run_command(command)),
        Err(message) => {
            eprintln!("{message}\n\nRun with --help for usage.");
            ExitCode::from(2)
        }
    }
}

fn parse_args(args: Vec<String>) -> Result<Command, String> {
    match args.as_slice() {
        [] => Ok(Command::Gui),
        [arg] if arg == "--help" || arg == "-h" => Ok(Command::Help),
        [arg] if arg == "--mock-preview" => Ok(Command::MockPreview),
        [flag, title_id] if flag == "--artwork-diagnostics" => Ok(Command::ArtworkDiagnostics {
            title_id: title_id.clone(),
        }),
        [flag, title_id] if flag == "--discord-test" => Ok(Command::DiscordTest {
            title_id: title_id.clone(),
            duration: 60,
        }),
        [flag, title_id, duration_flag, duration]
            if flag == "--discord-test" && duration_flag == "--duration-seconds" =>
        {
            let duration = duration
                .parse::<u64>()
                .map_err(|_| "duration must be an integer from 1 to 300 seconds".to_owned())?;
            if !(1..=300).contains(&duration) {
                return Err("duration must be from 1 to 300 seconds".to_owned());
            }
            Ok(Command::DiscordTest {
                title_id: title_id.clone(),
                duration,
            })
        }
        _ => Err("invalid command arguments".to_owned()),
    }
}

fn run_command(command: Command) -> u8 {
    match command {
        Command::Gui => {
            if let Err(error) = xerust360_rpc::ui::run() {
                eprintln!("XeRust360-RPC could not start: {error}");
                1
            } else {
                0
            }
        }
        Command::Help => {
            print_help();
            0
        }
        Command::MockPreview => {
            print_mock_preview();
            0
        }
        Command::ArtworkDiagnostics { title_id } => {
            let Some(report) = run_artwork_diagnostics(&title_id) else {
                return 1;
            };
            if report.selected.is_some()
                && report.metadata_cache_after == "write and readback passed"
            {
                println!("RESULT: PASS — live provider artwork and cache validated.");
                0
            } else {
                println!("RESULT: FAIL — no exact provider cover passed validation.");
                1
            }
        }
        Command::DiscordTest { title_id, duration } => {
            if run_discord_test(&title_id, duration) {
                0
            } else {
                1
            }
        }
    }
}

fn run_artwork_diagnostics(title_id: &str) -> Option<ArtworkLiveCheck> {
    println!("LIVE ARTWORK DIAGNOSTIC — network requests enabled; no mock data");
    println!("Observed input Title ID: {title_id}");
    println!("Only the Title ID is sent to public artwork providers; no credentials are sent.");
    let report = match artwork::live_diagnostic(title_id, &artwork::system_cache_dir()) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("Diagnostic could not run: {error}");
            return None;
        }
    };
    print_artwork_report(&report);
    Some(report)
}

fn print_artwork_report(report: &ArtworkLiveCheck) {
    println!("Normalized Title ID: {}", report.title_id);
    for provider in &report.providers {
        println!("\nProvider: {}", provider.provider);
        println!("  Metadata: {}", provider.metadata);
        if let Some(http) = &provider.metadata_http {
            print_http_check("Metadata HTTP", http);
        }
        println!(
            "  Exact title: {}",
            provider.title.as_deref().unwrap_or("unresolved")
        );
        println!(
            "  Cover URL: {}",
            provider.cover_url.as_deref().unwrap_or("none")
        );
        if let Some(cover) = &provider.cover {
            if let Some(http) = &cover.http {
                print_http_check("Cover HTTP", http);
            }
            println!("  Cover validation: {}", cover.validation);
            println!("  Cover cache: {}", cover.cache);
        }
    }
    println!("\nMetadata cache before: {}", report.metadata_cache_before);
    println!("Metadata cache after: {}", report.metadata_cache_after);
    if let Some(metadata) = &report.selected {
        println!(
            "Resolved title: {}",
            metadata.name.as_deref().unwrap_or("unavailable")
        );
        println!("Artwork provider: {}", metadata.source);
        println!(
            "Selected public image URL: {}",
            metadata.cover_url.as_deref().unwrap_or("none")
        );
    } else {
        println!("Resolved title: unresolved; no game name or cover was inferred.");
    }
}

fn print_http_check(label: &str, check: &artwork::ArtworkHttpCheck) {
    println!(
        "  {label}: HTTP {}; MIME {}; redirects {}; final URL {}; bytes {}",
        check.status, check.content_type, check.redirects, check.final_url, check.bytes
    );
}

fn run_discord_test(title_id: &str, duration_seconds: u64) -> bool {
    println!("DISCORD RPC TEST — live artwork required; no mock data");
    let Some(report) = run_artwork_diagnostics(title_id) else {
        return false;
    };
    let Some(metadata) = report.selected else {
        eprintln!("Refusing to publish: no live exact-ID metadata with validated cover art.");
        return false;
    };
    let Some(name) = metadata.name.clone() else {
        eprintln!("Refusing to publish: provider did not return an exact game title.");
        return false;
    };
    let Some(image_url) = metadata.cover_url.clone() else {
        eprintln!("Refusing to publish: provider did not return an external cover URL.");
        return false;
    };
    let application_id = settings::load().discord_application_id;
    if application_id.is_empty() {
        eprintln!("Set the Discord Application ID in app settings before running this mode.");
        return false;
    }
    let snapshot = TelemetrySnapshot {
        game: GameIdentity {
            title_id: Some(report.title_id.clone()),
            name: Some(name),
            artwork_title: metadata.name,
            cover_url: Some(image_url),
            artwork_source: Some(metadata.source),
            cached_cover_path: metadata.cached_cover_path,
            artwork_fallback_reason: None,
        },
        ..TelemetrySnapshot::default()
    };
    let presence = match format_presence(&snapshot, StateToggles::default(), Some(Duration::ZERO)) {
        Some(presence) => presence,
        None => {
            eprintln!("Refusing to publish: live artwork did not produce a valid presence.");
            return false;
        }
    };
    let mut client = match DiscordRpcClient::connect(&application_id) {
        Ok(client) => client,
        Err(error) => {
            eprintln!("Discord IPC connection/handshake failed: {error}");
            return false;
        }
    };
    if let Err(error) = client.set_presence(&presence) {
        eprintln!("Discord RPC response: {error}");
        return false;
    }
    println!("IPC RESPONSE: SET_ACTIVITY accepted by Discord.");
    println!("VISUAL RENDER: not verified by this command.");
    println!(
        "Inspect Discord's profile/activity card now; presence remains for {duration_seconds} seconds."
    );
    std::thread::sleep(Duration::from_secs(duration_seconds));
    match client.clear_presence() {
        Ok(()) => {
            println!("IPC RESPONSE: presence clear accepted by Discord.");
            true
        }
        Err(error) => {
            eprintln!("Discord RPC clear response failed: {error}");
            false
        }
    }
}

fn print_mock_preview() {
    let snapshot = sample_snapshot();
    let presence = format_presence(&snapshot, StateToggles::default(), Some(Duration::ZERO))
        .expect("mock snapshot contains a running title");
    println!("Playing {}", presence.name);
    println!("Details: {}", presence.details);
    println!("State: {}", presence.state);
    println!(
        "Title ID: {} (mock data)",
        snapshot.game.title_id.unwrap_or_default()
    );
}

fn print_help() {
    println!("XeRust360-RPC");
    println!("  (no arguments)                     Open the desktop app");
    println!("  --mock-preview                     Print synthetic preview data");
    println!("  --artwork-diagnostics <TITLE_ID>   Query live artwork providers and cache checks");
    println!("  --discord-test <TITLE_ID> [--duration-seconds N]");
    println!(
        "                                     Publish validated live artwork over local IPC, then clear"
    );
    println!("  --help                             Show this help");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cli_modes_are_opt_in_and_discord_interval_is_bounded() {
        assert_eq!(parse_args(vec![]), Ok(Command::Gui));
        assert_eq!(
            parse_args(vec!["--artwork-diagnostics".into(), "4D5307E6".into()]),
            Ok(Command::ArtworkDiagnostics {
                title_id: "4D5307E6".into()
            })
        );
        assert_eq!(
            parse_args(vec!["--discord-test".into(), "584111F7".into()]),
            Ok(Command::DiscordTest {
                title_id: "584111F7".into(),
                duration: 60
            })
        );
        assert!(
            parse_args(vec![
                "--discord-test".into(),
                "4D5307E6".into(),
                "--duration-seconds".into(),
                "301".into()
            ])
            .is_err()
        );
    }
}
