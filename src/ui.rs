use std::{
    net::{IpAddr, SocketAddr},
    str::FromStr,
    sync::mpsc::{self, Receiver, Sender},
    time::{Duration, Instant},
};

use eframe::egui;

use crate::{
    companion::SharedKey,
    credentials,
    domain::TelemetrySnapshot,
    nova::NovaCredentials,
    presence::{ACTIVITY_NAME, format_presence},
    settings::{self, AppSettings},
    worker::{AppWorker, WorkerCommand, WorkerEvent, WorkerSettings},
};

#[derive(Debug)]
enum TrayAction {
    Show,
    TogglePause,
    Quit,
}

#[cfg(target_os = "linux")]
#[derive(Clone, Debug)]
struct LinuxTray {
    actions: Sender<TrayAction>,
}

#[cfg(target_os = "linux")]
impl ksni::Tray for LinuxTray {
    fn id(&self) -> String {
        "xerust360-rpc".to_owned()
    }
    fn title(&self) -> String {
        "XeRust360-RPC".to_owned()
    }
    fn icon_name(&self) -> String {
        "input-gaming".to_owned()
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::StandardItem;
        let show = self.actions.clone();
        let pause = self.actions.clone();
        let quit = self.actions.clone();
        vec![
            StandardItem {
                label: "Show XeRust360-RPC".to_owned(),
                icon_name: "window-restore".to_owned(),
                activate: Box::new(move |_| {
                    let _ = show.send(TrayAction::Show);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Pause / Resume Presence".to_owned(),
                activate: Box::new(move |_| {
                    let _ = pause.send(TrayAction::TogglePause);
                }),
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
            StandardItem {
                label: "Quit".to_owned(),
                icon_name: "application-exit".to_owned(),
                activate: Box::new(move |_| {
                    let _ = quit.send(TrayAction::Quit);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

#[cfg(target_os = "linux")]
type TrayHandle = ksni::blocking::Handle<LinuxTray>;

pub struct DesktopApp {
    settings: AppSettings,
    worker: AppWorker,
    snapshot: Option<TelemetrySnapshot>,
    elapsed: Option<Duration>,
    snapshot_received_at: Option<Instant>,
    connection_status: String,
    discord_status: String,
    diagnostics: String,
    username: String,
    password: String,
    companion_shared_key: String,
    show_companion_key: bool,
    active_application_id: String,
    presence_paused: bool,
    discovery_results: Vec<IpAddr>,
    save_error: Option<String>,
    artwork_texture: Option<(std::path::PathBuf, egui::TextureHandle)>,
    tray_rx: Receiver<TrayAction>,
    #[cfg(target_os = "linux")]
    tray_handle: Option<TrayHandle>,
}

impl DesktopApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        cc.egui_ctx.set_visuals(egui::Visuals::dark());
        let settings = settings::load();
        let companion_shared_key = credentials::retrieve("peerpressure-companion-v1", "shared-key")
            .ok()
            .flatten()
            .unwrap_or_default();
        let worker_settings = WorkerSettings {
            mock_mode: settings.mock_mode,
            companion_mode: settings.companion_mode,
            auto_reconnect: settings.auto_reconnect,
            poll_interval: Duration::from_secs(settings.poll_interval_seconds.clamp(1, 120)),
            discord_application_id: settings.discord_application_id.clone(),
            toggles: settings.toggles,
            presence_paused: false,
        };
        let worker = AppWorker::start(worker_settings);
        let (tray_tx, tray_rx) = mpsc::channel();
        #[cfg(target_os = "linux")]
        let tray_handle = {
            use ksni::blocking::TrayMethods;
            LinuxTray { actions: tray_tx }.spawn().ok()
        };
        #[cfg(not(target_os = "linux"))]
        let _ = tray_tx;
        let should_auto_connect = !settings.mock_mode
            && !settings.companion_mode
            && !settings.nova_host.trim().is_empty();
        let should_start_companion = settings.companion_mode
            && !settings.companion_bind_address.trim().is_empty()
            && !companion_shared_key.is_empty();
        let mut app = Self {
            username: settings.nova_username.clone(),
            active_application_id: settings.discord_application_id.clone(),
            settings,
            worker,
            snapshot: None,
            elapsed: None,
            snapshot_received_at: None,
            connection_status: "Starting".to_owned(),
            discord_status: "Checking Discord".to_owned(),
            diagnostics: "Mock preview is available on first launch".to_owned(),
            password: String::new(),
            companion_shared_key,
            show_companion_key: false,
            presence_paused: false,
            discovery_results: Vec::new(),
            save_error: None,
            artwork_texture: None,
            tray_rx,
            #[cfg(target_os = "linux")]
            tray_handle,
        };
        if should_auto_connect {
            app.connect();
        } else if should_start_companion {
            app.start_companion();
        } else if app.settings.companion_mode {
            app.diagnostics =
                "Select a private LAN address and companion key to start the listener".to_owned();
        }
        app
    }

    fn worker_settings(&self) -> WorkerSettings {
        WorkerSettings {
            mock_mode: self.settings.mock_mode,
            companion_mode: self.settings.companion_mode,
            auto_reconnect: self.settings.auto_reconnect,
            poll_interval: Duration::from_secs(self.settings.poll_interval_seconds.clamp(1, 120)),
            discord_application_id: self.active_application_id.clone(),
            toggles: self.settings.toggles,
            presence_paused: self.presence_paused,
        }
    }

    fn save(&mut self) {
        match settings::save(&self.settings) {
            Ok(()) => self.save_error = None,
            Err(error) => self.save_error = Some(format!("Could not save preferences: {error}")),
        }
    }

    fn connect(&mut self) {
        let host = match IpAddr::from_str(self.settings.nova_host.trim()) {
            Ok(host) => host,
            Err(_) => {
                self.diagnostics = "Enter a valid console IP address".to_owned();
                return;
            }
        };
        let username = self.username.trim().to_owned();
        let credentials = if username.is_empty() {
            None
        } else {
            let password = if self.password.is_empty() {
                match credentials::retrieve(&host.to_string(), &username) {
                    Ok(Some(password)) => password,
                    Ok(None) => {
                        self.diagnostics =
                            "No saved NOVA password. Enter one or leave username blank.".to_owned();
                        return;
                    }
                    Err(error) => {
                        self.diagnostics = format!("Credential store: {error}");
                        return;
                    }
                }
            } else {
                let password = self.password.clone();
                if let Err(error) = credentials::store(&host.to_string(), &username, &password) {
                    self.diagnostics = format!(
                        "Credential store unavailable ({error}); using password for this session only."
                    );
                } else {
                    self.password.clear();
                }
                password
            };
            Some(NovaCredentials {
                username: username.clone(),
                password,
            })
        };
        self.settings.nova_host = host.to_string();
        self.settings.nova_username = username;
        self.settings.mock_mode = false;
        self.settings.companion_mode = false;
        self.save();
        self.worker
            .send(WorkerCommand::UpdateSettings(self.worker_settings()));
        self.worker.send(WorkerCommand::Connect {
            host,
            port: self.settings.nova_port,
            credentials,
        });
    }

    fn start_companion(&mut self) {
        let bind_ip = match IpAddr::from_str(self.settings.companion_bind_address.trim()) {
            Ok(address) => address,
            Err(_) => {
                self.diagnostics =
                    "Enter the desktop's private LAN address to listen on".to_owned();
                return;
            }
        };
        let key = match SharedKey::from_hex(self.companion_shared_key.trim()) {
            Ok(key) => key,
            Err(error) => {
                self.diagnostics = error.to_string();
                return;
            }
        };
        if let Err(error) = credentials::store(
            "peerpressure-companion-v1",
            "shared-key",
            self.companion_shared_key.trim(),
        ) {
            self.diagnostics = format!(
                "OS credential store unavailable ({error}); using the key for this session only"
            );
        }
        self.settings.mock_mode = false;
        self.settings.companion_mode = true;
        self.save();
        self.worker
            .send(WorkerCommand::UpdateSettings(self.worker_settings()));
        self.worker.send(WorkerCommand::StartCompanion {
            address: SocketAddr::new(bind_ip, self.settings.companion_port),
            key,
            heartbeat_timeout: Duration::from_secs(
                self.settings.companion_heartbeat_seconds.clamp(1, 60),
            ),
        });
    }

    fn process_events(&mut self) {
        while let Ok(event) = self.worker.try_event() {
            match event {
                WorkerEvent::Snapshot {
                    snapshot,
                    elapsed,
                    connection,
                    discord,
                    diagnostic,
                } => {
                    self.snapshot = Some(*snapshot);
                    self.elapsed = elapsed;
                    self.snapshot_received_at = Some(Instant::now());
                    self.connection_status = connection;
                    self.discord_status = discord;
                    self.diagnostics = diagnostic;
                }
                WorkerEvent::Status {
                    connection,
                    discord,
                    diagnostic,
                } => {
                    self.connection_status = connection;
                    self.discord_status = discord;
                    if !diagnostic.is_empty() {
                        self.diagnostics = diagnostic;
                    }
                }
                WorkerEvent::Discovered(results) => {
                    self.discovery_results = results;
                    self.diagnostics = if self.discovery_results.is_empty() {
                        "No NOVA console found on detected private /24–/30 networks; enter its IP manually.".to_owned()
                    } else {
                        format!(
                            "Found {} possible NOVA endpoint(s). Select one below.",
                            self.discovery_results.len()
                        )
                    };
                }
            }
        }
    }

    fn process_tray(&mut self, ctx: &egui::Context) {
        while let Ok(action) = self.tray_rx.try_recv() {
            match action {
                TrayAction::Show => ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true)),
                TrayAction::TogglePause => {
                    self.presence_paused = !self.presence_paused;
                    self.worker
                        .send(WorkerCommand::UpdateSettings(self.worker_settings()));
                }
                TrayAction::Quit => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
            }
        }
    }

    fn render_connection(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) -> bool {
        let mut changed = false;
        ui.heading("Telemetry source");
        ui.horizontal(|ui| {
            if ui
                .selectable_label(self.settings.mock_mode, "Mock")
                .clicked()
            {
                self.settings.mock_mode = true;
                self.settings.companion_mode = false;
                self.worker.send(WorkerCommand::Disconnect);
                changed = true;
            }
            if ui
                .selectable_label(
                    !self.settings.mock_mode && !self.settings.companion_mode,
                    "Aurora NOVA",
                )
                .clicked()
            {
                self.settings.mock_mode = false;
                self.settings.companion_mode = false;
                self.worker.send(WorkerCommand::Disconnect);
                changed = true;
            }
            if ui
                .selectable_label(self.settings.companion_mode, "Peer Pressure companion")
                .clicked()
            {
                self.settings.mock_mode = false;
                self.settings.companion_mode = true;
                self.worker.send(WorkerCommand::Disconnect);
                changed = true;
            }
        });
        if self.settings.companion_mode {
            ui.horizontal(|ui| {
                ui.label("Desktop LAN address");
                changed |= ui
                    .add(
                        egui::TextEdit::singleline(&mut self.settings.companion_bind_address)
                            .hint_text("desktop private IPv4 address")
                            .desired_width(190.0),
                    )
                    .changed();
                ui.label("Port");
                changed |= ui
                    .add(egui::DragValue::new(&mut self.settings.companion_port).range(1..=65535))
                    .changed();
                ui.label("Expiry");
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut self.settings.companion_heartbeat_seconds)
                            .range(1..=60)
                            .suffix(" s"),
                    )
                    .changed();
            });
            ui.horizontal(|ui| {
                ui.label("Companion key");
                ui.add(
                    egui::TextEdit::singleline(&mut self.companion_shared_key)
                        .password(!self.show_companion_key)
                        .hint_text("64 hexadecimal characters")
                        .desired_width(225.0),
                );
                if ui.button("Generate").clicked() {
                    match SharedKey::generate() {
                        Ok(key) => self.companion_shared_key = key.to_hex(),
                        Err(error) => self.diagnostics = error.to_string(),
                    }
                }
                if ui.button("Save key").clicked() {
                    match SharedKey::from_hex(self.companion_shared_key.trim()) {
                        Ok(_) => match credentials::store(
                            "peerpressure-companion-v1",
                            "shared-key",
                            self.companion_shared_key.trim(),
                        ) {
                            Ok(()) => {
                                self.diagnostics =
                                    "Companion key saved in OS credential store".to_owned()
                            }
                            Err(error) => self.diagnostics = format!("Credential store: {error}"),
                        },
                        Err(error) => self.diagnostics = error.to_string(),
                    }
                }
                changed |= ui.checkbox(&mut self.show_companion_key, "Show").changed();
                if ui.button("Copy for DLL config").clicked() {
                    ctx.copy_text(self.companion_shared_key.clone());
                    self.diagnostics = "Companion key copied to clipboard".to_owned();
                }
            });
            ui.horizontal(|ui| {
                if ui.button("Start listener").clicked() {
                    self.start_companion();
                }
                if ui.button("Stop listener").clicked() {
                    self.worker.send(WorkerCommand::StopCompanion);
                }
                ui.small("Configure a title DLL separately; this app never installs or deploys Xbox files.");
            });
        } else {
            ui.heading("Aurora NOVA connection");
            ui.horizontal(|ui| {
                ui.label("IP address");
                changed |= ui
                    .add(
                        egui::TextEdit::singleline(&mut self.settings.nova_host)
                            .hint_text("192.168.1.50")
                            .desired_width(170.0),
                    )
                    .changed();
                ui.label("Port");
                changed |= ui
                    .add(egui::DragValue::new(&mut self.settings.nova_port).range(1..=65535))
                    .changed();
                if ui.button("Discover").clicked() {
                    self.worker.send(WorkerCommand::Discover {
                        port: self.settings.nova_port,
                    });
                    self.diagnostics = "Searching private local networks for NOVA…".to_owned();
                }
                if ui.button("Connect").clicked() {
                    self.connect();
                }
                if ui.button("Disconnect").clicked() {
                    self.worker.send(WorkerCommand::Disconnect);
                }
            });
            ui.horizontal(|ui| {
                ui.label("NOVA username");
                changed |= ui
                    .add(egui::TextEdit::singleline(&mut self.username).desired_width(150.0))
                    .changed();
                ui.label("Password");
                ui.add(
                    egui::TextEdit::singleline(&mut self.password)
                        .password(true)
                        .desired_width(150.0),
                );
                if ui.button("Forget saved password").clicked()
                    && let Ok(host) = IpAddr::from_str(self.settings.nova_host.trim())
                {
                    match credentials::delete(&host.to_string(), self.username.trim()) {
                        Ok(()) => self.diagnostics = "Saved NOVA password removed".to_owned(),
                        Err(error) => self.diagnostics = format!("Credential store: {error}"),
                    }
                }
            });
            ui.horizontal(|ui| {
                changed |= ui
                    .checkbox(&mut self.settings.auto_reconnect, "Auto-reconnect")
                    .changed();
                ui.label("Poll interval");
                changed |= ui
                    .add(
                        egui::DragValue::new(&mut self.settings.poll_interval_seconds)
                            .range(1..=120)
                            .suffix(" s"),
                    )
                    .changed();
            });
            if !self.discovery_results.is_empty() {
                ui.horizontal(|ui| {
                    ui.label("Discovered:");
                    for ip in &self.discovery_results {
                        if ui.button(ip.to_string()).clicked() {
                            self.settings.nova_host = ip.to_string();
                            changed = true;
                        }
                    }
                });
            }
        }
        changed
    }

    fn render_presence_settings(&mut self, ui: &mut egui::Ui) -> bool {
        let mut changed = false;
        ui.heading("Discord Rich Presence");
        ui.horizontal(|ui| {
            ui.label("Discord Application ID");
            changed |= ui
                .add(
                    egui::TextEdit::singleline(&mut self.settings.discord_application_id)
                        .hint_text("Create an app named Xbox 360")
                        .desired_width(220.0),
                )
                .changed();
            if ui.button("Apply").clicked() {
                self.active_application_id = self.settings.discord_application_id.trim().to_owned();
                self.worker
                    .send(WorkerCommand::UpdateSettings(self.worker_settings()));
                changed = true;
            }
        });
        ui.label("The registered Discord app name should be “Xbox 360”. This uses local Discord IPC and needs no user token.");
        ui.separator();
        ui.label("Show in State");
        ui.horizontal_wrapped(|ui| {
            changed |= ui
                .checkbox(&mut self.settings.toggles.gamertag, "Gamertag")
                .changed();
            changed |= ui
                .checkbox(&mut self.settings.toggles.achievements, "Achievements")
                .changed();
            changed |= ui
                .checkbox(&mut self.settings.toggles.game_gamerscore, "Game GS")
                .changed();
            changed |= ui
                .checkbox(&mut self.settings.toggles.total_gamerscore, "Total GS")
                .changed();
            changed |= ui
                .checkbox(&mut self.settings.toggles.cpu_temperature, "CPU °C")
                .changed();
            changed |= ui
                .checkbox(&mut self.settings.toggles.gpu_temperature, "GPU °C")
                .changed();
        });
        changed
    }

    fn render_preview(&mut self, ui: &mut egui::Ui) {
        ui.heading("Live preview");
        let presence = self.snapshot.as_ref().and_then(|snapshot| {
            let elapsed = self.elapsed.map(|duration| {
                duration
                    + self
                        .snapshot_received_at
                        .map(|at| at.elapsed())
                        .unwrap_or_default()
            });
            format_presence(snapshot, self.settings.toggles, elapsed)
        });
        let artwork_path = self
            .snapshot
            .as_ref()
            .and_then(|snapshot| snapshot.game.cached_cover_path.clone());
        if let Some(path) = artwork_path
            && self
                .artwork_texture
                .as_ref()
                .is_none_or(|(cached_path, _)| cached_path != &path)
            && let Ok(bytes) = std::fs::read(&path)
            && let Ok(image) = image::load_from_memory(&bytes)
        {
            let image = image.to_rgba8();
            let size = [image.width() as usize, image.height() as usize];
            let color_image = egui::ColorImage::from_rgba_unmultiplied(size, image.as_raw());
            let texture =
                ui.ctx()
                    .load_texture("game-cover-art", color_image, egui::TextureOptions::LINEAR);
            self.artwork_texture = Some((path, texture));
        }
        egui::Frame::group(ui.style()).show(ui, |ui| {
            ui.horizontal(|ui| {
                if let Some((_, texture)) = &self.artwork_texture {
                    ui.add(
                        egui::Image::new((texture.id(), egui::vec2(80.0, 80.0)))
                            .fit_to_exact_size(egui::vec2(80.0, 80.0)),
                    );
                } else {
                    ui.label("🎮");
                }
                ui.vertical(|ui| {
                    ui.strong(format!("Playing {ACTIVITY_NAME}"));
                    if let Some(presence) = presence {
                        ui.label(presence.details);
                        if !presence.state.is_empty() {
                            ui.label(presence.state);
                        }
                        if let Some(elapsed) = presence.elapsed {
                            ui.small(format!("Elapsed {}", format_duration(elapsed)));
                        }
                    } else {
                        ui.label("No running game data available");
                    }
                });
            });
            if let Some(game) = self.snapshot.as_ref().map(|snapshot| &snapshot.game) {
                ui.separator();
                ui.strong("Artwork diagnostics");
                ui.small(format!(
                    "Title ID: {}",
                    game.title_id.as_deref().unwrap_or("unavailable")
                ));
                ui.small(format!(
                    "Resolved title: {}",
                    game.artwork_title.as_deref().unwrap_or("unresolved")
                ));
                ui.small(format!(
                    "Provider: {}",
                    game.artwork_source.as_deref().unwrap_or("none")
                ));
                ui.small(format!(
                    "Image URL: {}",
                    game.cover_url
                        .as_deref()
                        .unwrap_or("none; using generic asset")
                ));
                ui.small(format!(
                    "Cache: {}",
                    if game.cached_cover_path.is_some() {
                        "local image cached"
                    } else if game.cover_url.is_some() {
                        "image not cached locally"
                    } else {
                        "no cached image"
                    }
                ));
                if let Some(reason) = &game.artwork_fallback_reason {
                    ui.small(format!("Fallback reason: {reason}"));
                }
            } else {
                ui.small("Artwork diagnostics: no active Title ID; using generic Xbox 360 asset");
            }
            ui.small("Small image: XeRust360-RPC project icon");
        });
    }
}

impl eframe::App for DesktopApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.process_events();
        self.process_tray(ctx);
        #[cfg(target_os = "linux")]
        if self.tray_handle.is_some() && ctx.input(|input| input.viewport().close_requested()) {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }

        let mut changed = false;
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("XeRust360-RPC");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let label = if self.presence_paused {
                        "Resume presence"
                    } else {
                        "Pause presence"
                    };
                    if ui.button(label).clicked() {
                        self.presence_paused = !self.presence_paused;
                        self.worker
                            .send(WorkerCommand::UpdateSettings(self.worker_settings()));
                    }
                });
            });
            ui.label("Xbox 360 telemetry for Discord, over your local network");
            ui.add_space(10.0);
            egui::Frame::group(ui.style()).show(ui, |ui| {
                changed |= self.render_connection(ui, ctx);
            });
            ui.add_space(8.0);
            egui::Frame::group(ui.style()).show(ui, |ui| {
                changed |= self.render_presence_settings(ui);
            });
            ui.add_space(8.0);
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.strong("NOVA");
                    ui.label(&self.connection_status);
                    ui.separator();
                    ui.strong("Discord");
                    ui.label(&self.discord_status);
                });
                self.render_preview(ui);
            });
            ui.add_space(8.0);
            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Diagnostics");
                    if ui.button("Copy").clicked() {
                        ctx.copy_text(format!(
                            "NOVA: {}\nDiscord: {}\n{}",
                            self.connection_status, self.discord_status, self.diagnostics
                        ));
                    }
                });
                ui.label(&self.diagnostics);
                #[cfg(target_os = "linux")]
                if self.tray_handle.is_none() {
                    ui.small("System tray unavailable; this window will close normally.");
                }
                #[cfg(not(target_os = "linux"))]
                ui.small("Closing this window exits the app on this platform.");
                if let Some(error) = &self.save_error {
                    ui.colored_label(egui::Color32::LIGHT_RED, error);
                }
            });
            if changed {
                self.save();
                self.worker
                    .send(WorkerCommand::UpdateSettings(self.worker_settings()));
            }
        });
        ctx.request_repaint_after(Duration::from_millis(500));
    }
}

impl Drop for DesktopApp {
    fn drop(&mut self) {
        self.worker.send(WorkerCommand::Stop);
    }
}

fn format_duration(duration: Duration) -> String {
    let seconds = duration.as_secs();
    format!(
        "{:02}:{:02}:{:02}",
        seconds / 3600,
        (seconds / 60) % 60,
        seconds % 60
    )
}

pub fn run() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([780.0, 760.0])
            .with_min_inner_size([650.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "XeRust360-RPC",
        options,
        Box::new(|cc| Ok(Box::new(DesktopApp::new(cc)))),
    )
}
