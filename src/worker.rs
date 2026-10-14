use std::{
    net::{IpAddr, SocketAddr},
    sync::mpsc::{self, Receiver, Sender, TryRecvError},
    thread,
    time::{Duration, Instant},
};

use crate::{
    companion::{CompanionEvent, CompanionServer, SharedKey},
    discord::{DiscordError, DiscordRpcClient},
    domain::{GameSession, StateToggles, TelemetrySnapshot},
    mock::sample_snapshot,
    nova::{
        NovaClient, NovaConfig, NovaCredentials, PollDiagnostics, RetrySchedule,
        discover_nova_endpoints,
    },
    presence::{PresencePayload, format_presence},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkerSettings {
    pub mock_mode: bool,
    pub companion_mode: bool,
    pub auto_reconnect: bool,
    pub poll_interval: Duration,
    pub discord_application_id: String,
    pub toggles: StateToggles,
    pub presence_paused: bool,
}

impl Default for WorkerSettings {
    fn default() -> Self {
        Self {
            mock_mode: true,
            companion_mode: false,
            auto_reconnect: true,
            poll_interval: Duration::from_secs(10),
            discord_application_id: String::new(),
            toggles: StateToggles::default(),
            presence_paused: false,
        }
    }
}

#[derive(Debug)]
pub enum WorkerCommand {
    UpdateSettings(WorkerSettings),
    Connect {
        host: IpAddr,
        port: u16,
        credentials: Option<NovaCredentials>,
    },
    StartCompanion {
        address: SocketAddr,
        key: SharedKey,
        heartbeat_timeout: Duration,
    },
    StopCompanion,
    Disconnect,
    Discover {
        port: u16,
    },
    Stop,
}

#[derive(Clone, Debug)]
pub enum WorkerEvent {
    Snapshot {
        snapshot: TelemetrySnapshot,
        elapsed: Option<Duration>,
        connection: String,
        discord: String,
        diagnostic: String,
    },
    Status {
        connection: String,
        discord: String,
        diagnostic: String,
    },
    Discovered(Vec<IpAddr>),
}

pub struct AppWorker {
    commands: Sender<WorkerCommand>,
    events: Receiver<WorkerEvent>,
}

impl AppWorker {
    pub fn start(initial: WorkerSettings) -> Self {
        let (command_tx, command_rx) = mpsc::channel();
        let (event_tx, event_rx) = mpsc::channel();
        thread::Builder::new()
            .name("xerust360-worker".to_owned())
            .spawn(move || run_worker(command_rx, event_tx, initial))
            .expect("failed to start telemetry worker");
        Self {
            commands: command_tx,
            events: event_rx,
        }
    }

    pub fn send(&self, command: WorkerCommand) {
        let _ = self.commands.send(command);
    }

    pub fn try_event(&self) -> Result<WorkerEvent, TryRecvError> {
        self.events.try_recv()
    }
}

struct State {
    settings: WorkerSettings,
    config: Option<NovaConfig>,
    client: Option<NovaClient>,
    connection: String,
    discord_status: String,
    diagnostic: String,
    discord: Option<DiscordRpcClient>,
    discord_id: String,
    session: GameSession,
    has_published: bool,
    failures: u32,
    next_poll: Instant,
    companion_server: Option<CompanionServer>,
}

fn run_worker(
    commands: Receiver<WorkerCommand>,
    events: Sender<WorkerEvent>,
    settings: WorkerSettings,
) {
    let mut state = State {
        settings,
        config: None,
        client: None,
        connection: "Mock preview".to_owned(),
        discord_status: "Set a Discord Application ID to publish".to_owned(),
        diagnostic: "Synthetic telemetry is active".to_owned(),
        discord: None,
        discord_id: String::new(),
        session: GameSession::new(),
        has_published: false,
        failures: 0,
        next_poll: Instant::now(),
        companion_server: None,
    };
    loop {
        let timeout = state
            .next_poll
            .saturating_duration_since(Instant::now())
            .min(Duration::from_millis(250));
        match commands.recv_timeout(timeout) {
            Ok(WorkerCommand::Stop) => break,
            Ok(command) => handle_command(command, &mut state, &events),
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if Instant::now() >= state.next_poll {
            poll_once(&mut state, &events);
        }
        process_companion_events(&mut state, &events);
    }
    if let Some(discord) = state.discord.as_mut() {
        let _ = discord.clear_presence();
    }
}

fn handle_command(command: WorkerCommand, state: &mut State, events: &Sender<WorkerEvent>) {
    match command {
        WorkerCommand::UpdateSettings(settings) => {
            if settings.discord_application_id != state.discord_id {
                state.discord = None;
                state.discord_id = settings.discord_application_id.clone();
                state.discord_status = if state.discord_id.is_empty() {
                    "Set a Discord Application ID to publish".to_owned()
                } else {
                    "Connecting to Discord".to_owned()
                };
            }
            state.settings = settings;
            if state.settings.mock_mode {
                state.client = None;
                state.config = None;
            }
            if state.settings.companion_mode {
                state.client = None;
                state.config = None;
            }
            if !state.settings.companion_mode && state.companion_server.is_some() {
                state.companion_server = None;
                state.clear_presence();
                state.session.observe_title(None, Instant::now());
            }
            if state.settings.mock_mode {
                state.connection = "Mock preview".to_owned();
                state.diagnostic = "Synthetic values are labeled as mock data".to_owned();
                state.next_poll = Instant::now();
            } else if state.client.is_none() {
                state.connection = "Not connected".to_owned();
                state.diagnostic = "Enter the console IP address and connect to NOVA".to_owned();
            }
        }
        WorkerCommand::Connect {
            host,
            port,
            credentials,
        } => {
            state.companion_server = None;
            state.settings.companion_mode = false;
            let config = NovaConfig {
                host,
                port,
                credentials,
            };
            state.client = Some(NovaClient::new(config.clone()));
            state.config = Some(config);
            state.settings.mock_mode = false;
            state.connection = format!("Connecting to {host}:{port}");
            state.diagnostic.clear();
            state.failures = 0;
            state.next_poll = Instant::now();
        }
        WorkerCommand::StartCompanion {
            address,
            key,
            heartbeat_timeout,
        } => {
            state.client = None;
            state.config = None;
            state.companion_server = None;
            state.clear_presence();
            state.session.observe_title(None, Instant::now());
            state.settings.mock_mode = false;
            state.settings.companion_mode = true;
            match CompanionServer::bind(address, key, heartbeat_timeout) {
                Ok(server) => {
                    state.connection =
                        format!("Listening for companion on {}", server.local_addr());
                    state.diagnostic =
                        "Waiting for a configured title DLL; no console deployment is performed"
                            .to_owned();
                    state.companion_server = Some(server);
                }
                Err(error) => {
                    state.connection = "Companion listener unavailable".to_owned();
                    state.diagnostic = error.to_string();
                    state.settings.companion_mode = false;
                }
            }
            state.next_poll = Instant::now() + Duration::from_millis(250);
        }
        WorkerCommand::StopCompanion => {
            state.companion_server = None;
            state.settings.companion_mode = false;
            state.clear_presence();
            state.session.observe_title(None, Instant::now());
            state.connection = "Companion listener stopped".to_owned();
            state.diagnostic.clear();
            let _ = events.send(WorkerEvent::Snapshot {
                snapshot: TelemetrySnapshot::default(),
                elapsed: None,
                connection: state.connection.clone(),
                discord: state.discord_status.clone(),
                diagnostic: state.diagnostic.clone(),
            });
            state.next_poll = Instant::now() + Duration::from_secs(60 * 60);
        }
        WorkerCommand::Disconnect => {
            state.client = None;
            state.config = None;
            state.companion_server = None;
            state.settings.companion_mode = false;
            state.connection = "Disconnected".to_owned();
            state.diagnostic.clear();
            state.clear_presence();
            state.session.observe_title(None, Instant::now());
            let _ = events.send(WorkerEvent::Snapshot {
                snapshot: TelemetrySnapshot::default(),
                elapsed: None,
                connection: state.connection.clone(),
                discord: state.discord_status.clone(),
                diagnostic: state.diagnostic.clone(),
            });
            state.next_poll = Instant::now() + Duration::from_secs(60 * 60);
        }
        WorkerCommand::Discover { port } => {
            let events = events.clone();
            thread::spawn(move || {
                let found = discover_nova_endpoints(port)
                    .into_iter()
                    .map(|endpoint| IpAddr::V4(endpoint.host))
                    .collect();
                let _ = events.send(WorkerEvent::Discovered(found));
            });
        }
        WorkerCommand::Stop => {}
    }
    send_status(state, events);
}

fn poll_once(state: &mut State, events: &Sender<WorkerEvent>) {
    if state.settings.companion_mode {
        state.next_poll = Instant::now() + Duration::from_millis(250);
        return;
    }
    if state.settings.mock_mode {
        let snapshot = sample_snapshot();
        update_and_publish(state, snapshot.clone());
        state.connection = "Mock preview".to_owned();
        state.diagnostic = "Synthetic values are labeled as mock data".to_owned();
        let _ = events.send(WorkerEvent::Snapshot {
            elapsed: state.session.elapsed(Instant::now()),
            snapshot,
            connection: state.connection.clone(),
            discord: state.discord_status.clone(),
            diagnostic: state.diagnostic.clone(),
        });
        state.next_poll = Instant::now() + Duration::from_secs(1);
        return;
    }
    let Some(client) = state.client.as_mut() else {
        state.next_poll = Instant::now() + Duration::from_secs(1);
        return;
    };
    match client.poll() {
        Ok(result) => {
            let mut snapshot = result.snapshot;
            if snapshot.game.name.is_none() {
                snapshot.game.name = snapshot
                    .game
                    .title_id
                    .as_deref()
                    .and_then(crate::catalog::resolve_title_name)
                    .map(str::to_owned);
            }
            update_and_publish(state, snapshot.clone());
            state.failures = 0;
            state.connection = "Connected to Aurora / NOVA".to_owned();
            state.diagnostic = diagnostics_text(&result.diagnostics);
            let elapsed = state.session.elapsed(Instant::now());
            let _ = events.send(WorkerEvent::Snapshot {
                snapshot,
                elapsed,
                connection: state.connection.clone(),
                discord: state.discord_status.clone(),
                diagnostic: state.diagnostic.clone(),
            });
            state.next_poll = Instant::now() + state.settings.poll_interval;
        }
        Err(error) => {
            state.failures = state.failures.saturating_add(1);
            state.connection = if state.settings.auto_reconnect {
                format!("Reconnecting (attempt {})", state.failures)
            } else {
                "Connection failed".to_owned()
            };
            state.diagnostic = error.to_string();
            state.clear_presence();
            send_status(state, events);
            state.next_poll = if state.settings.auto_reconnect {
                Instant::now() + RetrySchedule::default().delay(state.failures - 1)
            } else {
                Instant::now() + Duration::from_secs(60 * 60)
            };
        }
    }
}

fn process_companion_events(state: &mut State, events: &Sender<WorkerEvent>) {
    let queued: Vec<_> = state
        .companion_server
        .as_ref()
        .into_iter()
        .flat_map(|server| std::iter::from_fn(|| server.try_event().ok()))
        .collect();
    for event in queued {
        match event {
            CompanionEvent::Connected => {
                state.clear_presence();
                state.session.observe_title(None, Instant::now());
                state.connection = "Peer Pressure title companion connected".to_owned();
                state.diagnostic = "Receiving authenticated, read-only telemetry".to_owned();
                send_status(state, events);
            }
            CompanionEvent::Snapshot { snapshot, .. } => {
                let mut snapshot = snapshot;
                if snapshot.game.name.is_none() {
                    snapshot.game.name = snapshot
                        .game
                        .title_id
                        .as_deref()
                        .and_then(crate::catalog::resolve_title_name)
                        .map(str::to_owned);
                }
                update_and_publish(state, snapshot.clone());
                state.connection = "Peer Pressure title companion active".to_owned();
                state.diagnostic = "Values are reported by the current title DLL; unsupported fields remain unavailable".to_owned();
                let _ = events.send(WorkerEvent::Snapshot {
                    elapsed: state.session.elapsed(Instant::now()),
                    snapshot,
                    connection: state.connection.clone(),
                    discord: state.discord_status.clone(),
                    diagnostic: state.diagnostic.clone(),
                });
            }
            CompanionEvent::Disconnected(reason) => {
                state.clear_presence();
                state.session.observe_title(None, Instant::now());
                state.connection = "Peer Pressure title companion unavailable".to_owned();
                state.diagnostic = reason.to_owned();
                let _ = events.send(WorkerEvent::Snapshot {
                    snapshot: TelemetrySnapshot::default(),
                    elapsed: None,
                    connection: state.connection.clone(),
                    discord: state.discord_status.clone(),
                    diagnostic: state.diagnostic.clone(),
                });
            }
            CompanionEvent::Rejected(reason) => {
                state.diagnostic = reason.to_owned();
                send_status(state, events);
            }
        }
    }
}

impl State {
    fn clear_presence(&mut self) {
        if self.has_published {
            if let Some(client) = self.discord.as_mut() {
                let _ = client.clear_presence();
            }
            self.has_published = false;
        }
    }
}

fn update_and_publish(state: &mut State, snapshot: TelemetrySnapshot) {
    let title_key = snapshot
        .game
        .title_id
        .as_deref()
        .or(snapshot.game.name.as_deref());
    state.session.observe_title(title_key, Instant::now());
    let presence = format_presence(
        &snapshot,
        state.settings.toggles,
        state.session.elapsed(Instant::now()),
    );
    publish(state, presence.as_ref());
}

fn publish(state: &mut State, presence: Option<&PresencePayload>) {
    if state.settings.presence_paused {
        if state.has_published {
            if let Some(client) = state.discord.as_mut() {
                let _ = client.clear_presence();
            }
            state.has_published = false;
        }
        state.discord_status = "Presence paused".to_owned();
        return;
    }
    if state.discord_id.is_empty() {
        state.discord_status = "Set a Discord Application ID to publish".to_owned();
        return;
    }
    if state.discord.is_none() {
        match DiscordRpcClient::connect(&state.discord_id) {
            Ok(client) => {
                state.discord = Some(client);
                state.discord_status = "Connected to Discord".to_owned();
            }
            Err(error) => {
                state.discord_status = discord_error_text(&error);
                return;
            }
        }
    }
    let result = match (state.discord.as_mut(), presence) {
        (Some(client), Some(presence)) => client.set_presence(presence),
        (Some(client), None) if state.has_published => client.clear_presence(),
        (Some(_), None) => Ok(()),
        (None, _) => return,
    };
    match result {
        Ok(()) => {
            state.has_published = presence.is_some();
            state.discord_status = if presence.is_some() {
                "Presence updated".to_owned()
            } else {
                "No game to publish".to_owned()
            };
        }
        Err(error) => {
            state.discord_status = discord_error_text(&error);
            state.discord = None;
            state.has_published = false;
        }
    }
}

fn discord_error_text(error: &DiscordError) -> String {
    error.to_string()
}

fn diagnostics_text(diagnostics: &PollDiagnostics) -> String {
    let mut issues = Vec::new();
    for (name, result) in [
        ("title", &diagnostics.title),
        ("profile", &diagnostics.profile),
        ("achievements", &diagnostics.achievements),
        ("temperatures", &diagnostics.temperatures),
    ] {
        if let Err(error) = result {
            issues.push(format!("{name}: {error}"));
        }
    }
    if issues.is_empty() {
        "NOVA telemetry is current".to_owned()
    } else {
        issues.join("; ")
    }
}

fn send_status(state: &State, events: &Sender<WorkerEvent>) {
    let _ = events.send(WorkerEvent::Status {
        connection: state.connection.clone(),
        discord: state.discord_status.clone(),
        diagnostic: state.diagnostic.clone(),
    });
}
