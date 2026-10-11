use std::time::Duration;

use xerust360_rpc::{domain::StateToggles, mock::sample_snapshot, presence::format_presence};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--mock-preview") {
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
    } else {
        if let Err(error) = xerust360_rpc::ui::run() {
            eprintln!("XeRust360-RPC could not start: {error}");
            std::process::exit(1);
        }
    }
}
