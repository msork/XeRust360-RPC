//! Exact Title ID metadata and cover resolution with bounded external providers.
//!
//! Provider artwork is never bundled. Public URLs are passed to Discord; a
//! bounded local copy is retained for the desktop preview and offline cache.

use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};

pub const MAX_METADATA_BYTES: usize = 12 * 1024 * 1024;
pub const MAX_IMAGE_BYTES: usize = 4 * 1024 * 1024;
const METADATA_TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const NEGATIVE_TTL: Duration = Duration::from_secs(30 * 60);
const MAX_CACHE_ENTRIES: usize = 2_000;
const MAX_CACHE_BYTES: u64 = 512 * 1024 * 1024;
const DISCORD_IMAGE_URL_MAX_CHARS: usize = 300;
const MAX_REDIRECTS: u32 = 5;
const CURL_METADATA_MARKER: &str = "\n__XERUST_CURL_METADATA__";

#[derive(Debug, PartialEq, Eq)]
struct HttpResponse {
    body: Vec<u8>,
    status: u16,
    content_type: String,
    redirects: u32,
    final_url: String,
    transfer_ok: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtworkHttpCheck {
    pub status: u16,
    pub content_type: String,
    pub redirects: u32,
    pub final_url: String,
    pub bytes: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtworkCoverCheck {
    pub http: Option<ArtworkHttpCheck>,
    pub validation: String,
    pub cache: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtworkProviderCheck {
    pub provider: String,
    pub metadata: String,
    pub metadata_http: Option<ArtworkHttpCheck>,
    pub title: Option<String>,
    pub cover_url: Option<String>,
    pub cover: Option<ArtworkCoverCheck>,
    metadata_result: Option<GameMetadata>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtworkLiveCheck {
    pub title_id: String,
    pub providers: Vec<ArtworkProviderCheck>,
    pub metadata_cache_before: String,
    pub metadata_cache_after: String,
    pub selected: Option<GameMetadata>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GameMetadata {
    pub title_id: String,
    pub name: Option<String>,
    pub cover_url: Option<String>,
    pub source: String,
    /// `Some(true)` only when the provider explicitly marks the cover official.
    pub official_cover: Option<bool>,
    #[serde(default)]
    pub related_title_ids: Vec<String>,
    #[serde(default)]
    pub cached_cover_path: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ArtworkError {
    InvalidTitleId,
    Provider(String),
    Cache(String),
}

impl std::fmt::Display for ArtworkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidTitleId => f.write_str("invalid Xbox 360 Title ID"),
            Self::Provider(name) => write!(f, "artwork provider unavailable: {name}"),
            Self::Cache(name) => write!(f, "artwork cache unavailable: {name}"),
        }
    }
}

impl std::error::Error for ArtworkError {}

pub trait MetadataProvider: Send + Sync {
    fn name(&self) -> &'static str;
    fn lookup(&self, normalized_title_id: &str) -> Result<Option<GameMetadata>, ArtworkError>;
}

#[derive(Clone)]
pub struct ArtworkResolver {
    providers: Vec<Arc<dyn MetadataProvider>>,
    cache_dir: PathBuf,
    online: bool,
    cache_covers_enabled: bool,
}

impl ArtworkResolver {
    pub fn new(
        cache_dir: PathBuf,
        providers: Vec<Arc<dyn MetadataProvider>>,
        online: bool,
    ) -> Self {
        Self {
            providers,
            cache_dir,
            online,
            cache_covers_enabled: false,
        }
    }

    pub fn cache_covers(mut self, enabled: bool) -> Self {
        self.cache_covers_enabled = enabled;
        self
    }

    pub fn system_default() -> Self {
        let cache_dir = system_cache_dir();
        Self::new(
            cache_dir,
            vec![
                Arc::new(X360DbProvider::default()),
                Arc::new(XboxUnityProvider::default()),
            ],
            true,
        )
        .cache_covers(true)
    }

    pub fn resolve(&self, title_id: &str) -> Result<Option<GameMetadata>, ArtworkError> {
        let normalized =
            crate::catalog::normalize_title_id(title_id).ok_or(ArtworkError::InvalidTitleId)?;
        let cache_file = self.cache_dir.join(format!("{normalized}.json"));
        let cached = read_cache(&cache_file).ok();
        if let Some(entry) = cached.as_ref() {
            let age = unix_now().saturating_sub(entry.checked_at);
            let ttl = if entry.metadata.is_some() {
                METADATA_TTL
            } else {
                NEGATIVE_TTL
            };
            if !self.online || age <= ttl.as_secs() {
                let mut metadata = entry.metadata.clone();
                if let Some(game) = metadata.as_mut()
                    && game
                        .cached_cover_path
                        .as_ref()
                        .is_some_and(|path| !path.is_file() || !path.starts_with(&self.cache_dir))
                {
                    game.cached_cover_path = None;
                }
                if metadata
                    .as_ref()
                    .is_some_and(|game| game.title_id != normalized)
                {
                    metadata = None;
                }
                if let Some(game) = metadata.as_mut()
                    && !game
                        .cover_url
                        .as_deref()
                        .is_some_and(valid_discord_image_url)
                {
                    game.cover_url = None;
                }
                return Ok(metadata);
            }
        }
        if !self.online {
            return Ok(None);
        }

        let mut found: Option<GameMetadata> = None;
        let mut last_error = None;
        for provider in &self.providers {
            match provider.lookup(&normalized) {
                Ok(Some(mut metadata)) => {
                    if metadata.title_id != normalized
                        && !metadata.related_title_ids.contains(&normalized)
                    {
                        continue;
                    }
                    if !metadata
                        .cover_url
                        .as_deref()
                        .is_some_and(valid_discord_image_url)
                    {
                        metadata.cover_url = None;
                    }
                    metadata.title_id = normalized.clone();
                    if let Some(existing) = found.as_mut() {
                        if existing.name.is_none() {
                            existing.name = metadata.name;
                        }
                        let incoming_is_better = metadata.cover_url.is_some()
                            && (existing.cover_url.is_none()
                                || metadata.official_cover == Some(true)
                                    && existing.official_cover != Some(true));
                        if incoming_is_better {
                            existing.cover_url = metadata.cover_url;
                            existing.official_cover = metadata.official_cover;
                            existing.source = format!("{} + {}", existing.source, metadata.source);
                        } else if existing.source != metadata.source {
                            existing.source = format!("{} + {}", existing.source, metadata.source);
                        }
                        if existing.related_title_ids.is_empty() {
                            existing.related_title_ids = metadata.related_title_ids;
                        }
                    } else {
                        found = Some(metadata);
                    }
                    // A metadata-only hit can still be enriched by the next provider.
                    if found.as_ref().is_some_and(|game| {
                        game.name.is_some()
                            && game.cover_url.is_some()
                            && game.official_cover == Some(true)
                    }) {
                        break;
                    }
                }
                Ok(None) => {}
                Err(error) => last_error = Some(error),
            }
        }

        if self.cache_covers_enabled
            && let Some(metadata) = found.as_mut()
            && let Some(url) = metadata.cover_url.as_deref()
        {
            match fetch_and_cache_cover(url, &self.cache_dir, &normalized) {
                Ok(path) => metadata.cached_cover_path = Some(path),
                // Keep the public URL for Discord; local download/cache failure
                // does not prove that Discord's media proxy cannot reach it.
                Err(_) => metadata.cached_cover_path = None,
            }
        }

        if let Some(stale) = cached.as_ref().and_then(|entry| entry.metadata.as_ref()) {
            if let Some(metadata) = found.as_mut() {
                if metadata.name.is_none() {
                    metadata.name = stale.name.clone();
                }
                if metadata.cover_url.is_none() {
                    metadata.cover_url = stale.cover_url.clone();
                    metadata.official_cover = stale.official_cover;
                    metadata.cached_cover_path = stale.cached_cover_path.clone();
                }
            } else if last_error.is_some() {
                let mut metadata = stale.clone();
                if metadata
                    .cached_cover_path
                    .as_ref()
                    .is_some_and(|path| !path.is_file() || !path.starts_with(&self.cache_dir))
                {
                    metadata.cached_cover_path = None;
                }
                if !metadata
                    .cover_url
                    .as_deref()
                    .is_some_and(valid_discord_image_url)
                {
                    metadata.cover_url = None;
                }
                return Ok(Some(metadata));
            }
        }

        if found.is_none()
            && let Some(error) = last_error
        {
            return Err(error);
        }

        let entry = CacheEntry {
            checked_at: unix_now(),
            metadata: found.clone(),
        };
        write_cache(&cache_file, &entry).map_err(|error| ArtworkError::Cache(error.to_string()))?;
        prune_cache(&self.cache_dir);
        Ok(found)
    }
}

pub fn system_cache_dir() -> PathBuf {
    crate::settings::settings_path()
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("artwork")
}

/// Performs explicit live checks against both built-in metadata providers and
/// validates each returned image before writing a diagnostic cache entry.
/// This never falls back to mock metadata.
pub fn live_diagnostic(title_id: &str, cache_dir: &Path) -> Result<ArtworkLiveCheck, ArtworkError> {
    let normalized =
        crate::catalog::normalize_title_id(title_id).ok_or(ArtworkError::InvalidTitleId)?;
    let metadata_path = cache_dir.join(format!("{normalized}.json"));
    let metadata_cache_before = if read_cache(&metadata_path).is_ok() {
        "hit (existing metadata cache entry)".to_owned()
    } else {
        "miss".to_owned()
    };
    let diagnostic_dir = cache_dir.join("diagnostics");
    let mut providers = vec![probe_x360db(&normalized), probe_xboxunity(&normalized)];

    for (index, result) in providers.iter_mut().enumerate() {
        let Some(metadata) = result.metadata_result.clone() else {
            continue;
        };
        let Some(url) = metadata.cover_url.as_deref() else {
            result.cover = Some(ArtworkCoverCheck {
                http: None,
                validation: "provider returned no usable cover URL".to_owned(),
                cache: "not written".to_owned(),
            });
            continue;
        };
        match curl_image(url) {
            Ok(response) => {
                let http = Some(http_check(&response));
                if let Some(reason) = cover_response_error(&response) {
                    result.cover = Some(ArtworkCoverCheck {
                        http,
                        validation: format!("failed: {reason}"),
                        cache: "not written".to_owned(),
                    });
                    continue;
                }
                let key = format!(
                    "{}-{}",
                    normalized,
                    if index == 0 { "x360db" } else { "xboxunity" }
                );
                match cache_cover_bytes(&response.body, &diagnostic_dir, &key) {
                    Ok(path) => {
                        let cache_ok = fs::read(path).is_ok_and(|cached| cached == response.body);
                        result.cover = Some(ArtworkCoverCheck {
                            http,
                            validation: "passed: HTTPS, MIME, size, and image signature".to_owned(),
                            cache: if cache_ok {
                                "write and readback passed".to_owned()
                            } else {
                                "write/readback failed".to_owned()
                            },
                        });
                    }
                    Err(error) => {
                        result.cover = Some(ArtworkCoverCheck {
                            http,
                            validation: "passed: HTTPS, MIME, size, and image signature".to_owned(),
                            cache: if error.kind() == io::ErrorKind::PermissionDenied {
                                "write failed: permission denied".to_owned()
                            } else {
                                "write failed".to_owned()
                            },
                        });
                    }
                }
            }
            Err(error) => {
                result.cover = Some(ArtworkCoverCheck {
                    http: None,
                    validation: format!("failed: {error}"),
                    cache: "not written".to_owned(),
                });
            }
        }
    }

    let mut selected: Option<GameMetadata> = None;
    for provider in &providers {
        let Some(metadata) = &provider.metadata_result else {
            continue;
        };
        if provider
            .cover
            .as_ref()
            .is_none_or(|cover| !cover.cache.starts_with("write and readback passed"))
        {
            continue;
        }
        match selected.as_mut() {
            None => selected = Some(metadata.clone()),
            Some(existing) => {
                if existing.name.is_none() {
                    existing.name = metadata.name.clone();
                }
                if metadata.official_cover == Some(true) && existing.official_cover != Some(true) {
                    let name = existing.name.clone().or_else(|| metadata.name.clone());
                    *existing = metadata.clone();
                    existing.name = name;
                }
            }
        }
    }
    if let Some(title) = providers
        .iter()
        .filter_map(|provider| provider.metadata_result.as_ref())
        .find_map(|metadata| metadata.name.clone())
        && let Some(selected) = selected.as_mut()
    {
        selected.name = Some(title);
    }

    let metadata_cache_after = if let Some(mut metadata) = selected.clone() {
        let source_cover = providers
            .iter()
            .find(|provider| {
                provider
                    .metadata_result
                    .as_ref()
                    .is_some_and(|candidate| candidate.source == metadata.source)
                    && provider
                        .cover
                        .as_ref()
                        .is_some_and(|cover| cover.cache.starts_with("write and readback passed"))
            })
            .and_then(|provider| {
                let key = if provider.provider == "x360db" {
                    "x360db"
                } else {
                    "xboxunity"
                };
                fs::read(diagnostic_dir.join(format!("{normalized}-{key}.cover"))).ok()
            });
        if let Some(bytes) = source_cover
            && let Ok(path) = cache_cover_bytes(&bytes, cache_dir, &normalized)
        {
            metadata.cached_cover_path = Some(path);
        }
        let entry = CacheEntry {
            checked_at: unix_now(),
            metadata: Some(metadata.clone()),
        };
        if write_cache(&metadata_path, &entry).is_ok()
            && read_cache(&metadata_path)
                .is_ok_and(|cached| cached.metadata.as_ref() == Some(&metadata))
        {
            "write and readback passed".to_owned()
        } else {
            "write/readback failed".to_owned()
        }
    } else {
        "not written: no exact title with a validated cover".to_owned()
    };

    Ok(ArtworkLiveCheck {
        title_id: normalized,
        providers,
        metadata_cache_before,
        metadata_cache_after,
        selected,
    })
}

fn probe_x360db(title_id: &str) -> ArtworkProviderCheck {
    let provider = "x360db";
    let response = curl_get(
        "https://xenia-manager.github.io/x360db/games.json",
        MAX_METADATA_BYTES,
    );
    let response = match response {
        Ok(response) => response,
        Err(error) => {
            return provider_check(provider, &format!("failed: {error}"), None, None);
        }
    };
    let Some(response) = response else {
        return provider_check(provider, "HTTP not found", None, None);
    };
    let http = http_check(&response);
    if let Some(reason) = metadata_response_error(&response) {
        return provider_check(
            provider,
            &format!("failed: HTTP {}; {reason}", response.status),
            Some(http),
            None,
        );
    }
    let entries: Vec<X360DbEntry> = match serde_json::from_slice(&response.body) {
        Ok(entries) => entries,
        Err(_) => {
            return provider_check(provider, "failed: invalid JSON response", Some(http), None);
        }
    };
    let metadata = find_x360db_title(&entries, title_id).map(|mut metadata| {
        metadata.title_id = title_id.to_owned();
        metadata
    });
    let status = if metadata.is_some() {
        "HTTP 200; exact Title ID matched"
    } else {
        "HTTP 200; no exact Title ID match"
    };
    provider_check(provider, status, Some(http), metadata)
}

fn probe_xboxunity(title_id: &str) -> ArtworkProviderCheck {
    let provider = "XboxUnity";
    let endpoint = format!("https://xboxunity.net/Resources/Lib/CoverInfo.php?titleid={title_id}");
    let response = match curl_get(&endpoint, MAX_METADATA_BYTES) {
        Ok(response) => response,
        Err(error) => {
            return provider_check(provider, &format!("failed: {error}"), None, None);
        }
    };
    let Some(response) = response else {
        return provider_check(provider, "HTTP not found", None, None);
    };
    let http = http_check(&response);
    if let Some(reason) = metadata_response_error(&response) {
        return provider_check(
            provider,
            &format!("failed: HTTP {}; {reason}", response.status),
            Some(http),
            None,
        );
    }
    let metadata = match parse_xboxunity_covers(&response.body, title_id) {
        Ok(metadata) => metadata,
        Err(_) => {
            return provider_check(
                provider,
                "failed: invalid cover metadata JSON",
                Some(http),
                None,
            );
        }
    };
    let status = if metadata.is_some() {
        "HTTP 200; cover metadata matched exact Title ID"
    } else {
        "HTTP 200; no cover metadata for exact Title ID"
    };
    provider_check(provider, status, Some(http), metadata)
}

fn provider_check(
    provider: &str,
    metadata_status: &str,
    metadata_http: Option<ArtworkHttpCheck>,
    metadata: Option<GameMetadata>,
) -> ArtworkProviderCheck {
    ArtworkProviderCheck {
        provider: provider.to_owned(),
        metadata: metadata_status.to_owned(),
        metadata_http,
        title: metadata.as_ref().and_then(|game| game.name.clone()),
        cover_url: metadata.as_ref().and_then(|game| game.cover_url.clone()),
        cover: None,
        metadata_result: metadata,
    }
}

fn http_check(response: &HttpResponse) -> ArtworkHttpCheck {
    ArtworkHttpCheck {
        status: response.status,
        content_type: response.content_type.clone(),
        redirects: response.redirects,
        final_url: response.final_url.clone(),
        bytes: response.body.len(),
    }
}

#[derive(Serialize, Deserialize)]
struct CacheEntry {
    checked_at: u64,
    metadata: Option<GameMetadata>,
}

#[derive(Deserialize)]
struct X360DbEntry {
    id: String,
    #[serde(default)]
    alternative_id: Vec<String>,
    title: String,
    boxart: Option<String>,
}

fn find_x360db_title(entries: &[X360DbEntry], title_id: &str) -> Option<GameMetadata> {
    entries
        .iter()
        .find(|entry| {
            crate::catalog::normalize_title_id(&entry.id).as_deref() == Some(title_id)
                || entry
                    .alternative_id
                    .iter()
                    .filter_map(|id| crate::catalog::normalize_title_id(id))
                    .any(|id| id == title_id)
        })
        .map(|entry| {
            let id = crate::catalog::normalize_title_id(&entry.id).unwrap_or_default();
            let cover_url = entry.boxart.as_ref().map(|url| {
                if valid_discord_image_url(url) {
                    url.clone()
                } else {
                    format!("https://xenia-manager.github.io/x360db/titles/{id}/artwork/boxart.jpg")
                }
            });
            GameMetadata {
                title_id: id,
                name: Some(entry.title.clone()),
                cover_url,
                source: "x360db".to_owned(),
                official_cover: None,
                related_title_ids: entry
                    .alternative_id
                    .iter()
                    .filter_map(|id| crate::catalog::normalize_title_id(id))
                    .collect(),
                cached_cover_path: None,
            }
        })
}

#[derive(Deserialize)]
struct XboxUnityCoversResponse {
    #[serde(rename = "Covers", default)]
    covers: Vec<XboxUnityCover>,
}

#[derive(Deserialize)]
struct XboxUnityCover {
    #[serde(rename = "CoverID")]
    cover_id: String,
    #[serde(rename = "Official", default)]
    official: String,
}

fn parse_xboxunity_covers(
    bytes: &[u8],
    title_id: &str,
) -> Result<Option<GameMetadata>, ArtworkError> {
    let response: XboxUnityCoversResponse = serde_json::from_slice(bytes)
        .map_err(|_| ArtworkError::Provider("XboxUnity".to_owned()))?;
    let cover = response
        .covers
        .iter()
        .find(|cover| cover.official == "1")
        .or_else(|| response.covers.first());
    let Some(cover) = cover else {
        return Ok(None);
    };
    let cover_id = cover
        .cover_id
        .parse::<u32>()
        .map_err(|_| ArtworkError::Provider("XboxUnity".to_owned()))?;
    Ok(Some(GameMetadata {
        title_id: title_id.to_owned(),
        name: None,
        cover_url: Some(format!(
            "https://xboxunity.net/Resources/Lib/Cover.php?cid={cover_id}&size=large"
        )),
        source: "XboxUnity".to_owned(),
        official_cover: Some(cover.official == "1"),
        related_title_ids: Vec::new(),
        cached_cover_path: None,
    }))
}

#[derive(Default)]
pub struct X360DbProvider {
    limiter: RequestLimiter,
    index: Mutex<Option<(std::time::Instant, Vec<X360DbEntry>)>>,
}

impl MetadataProvider for X360DbProvider {
    fn name(&self) -> &'static str {
        "x360db"
    }

    fn lookup(&self, title_id: &str) -> Result<Option<GameMetadata>, ArtworkError> {
        let entries = {
            let mut index = self
                .index
                .lock()
                .map_err(|_| ArtworkError::Provider(self.name().to_owned()))?;
            let is_fresh = index
                .as_ref()
                .is_some_and(|(loaded_at, _)| loaded_at.elapsed() < METADATA_TTL);
            if !is_fresh {
                self.limiter.wait();
                let Some(response) = curl_get(
                    "https://xenia-manager.github.io/x360db/games.json",
                    MAX_METADATA_BYTES,
                )?
                else {
                    return Ok(None);
                };
                if response.status == 404 || response.status == 410 {
                    return Ok(None);
                }
                if let Some(reason) = metadata_response_error(&response) {
                    return Err(ArtworkError::Provider(format!("{}: {reason}", self.name())));
                }
                let parsed: Vec<X360DbEntry> = serde_json::from_slice(&response.body)
                    .map_err(|_| ArtworkError::Provider(self.name().to_owned()))?;
                *index = Some((std::time::Instant::now(), parsed));
            }
            index
                .as_ref()
                .and_then(|(_, entries)| find_x360db_title(entries, title_id))
        };
        let Some(mut metadata) = entries else {
            return Ok(None);
        };
        // The index may resolve an alternate/disc ID to its canonical entry.
        if metadata.title_id != title_id
            && !metadata.related_title_ids.iter().any(|id| id == title_id)
        {
            return Ok(None);
        }
        metadata.title_id = title_id.to_owned();
        Ok(Some(metadata))
    }
}

#[derive(Default)]
pub struct XboxUnityProvider {
    limiter: RequestLimiter,
}

impl MetadataProvider for XboxUnityProvider {
    fn name(&self) -> &'static str {
        "XboxUnity"
    }

    fn lookup(&self, title_id: &str) -> Result<Option<GameMetadata>, ArtworkError> {
        self.limiter.wait();
        let endpoint =
            format!("https://xboxunity.net/Resources/Lib/CoverInfo.php?titleid={title_id}");
        let Some(response) = curl_get(&endpoint, MAX_METADATA_BYTES)? else {
            return Ok(None);
        };
        if response.status == 404 || response.status == 410 {
            return Ok(None);
        }
        if let Some(reason) = metadata_response_error(&response) {
            return Err(ArtworkError::Provider(format!("{}: {reason}", self.name())));
        }
        parse_xboxunity_covers(&response.body, title_id)
    }
}

#[derive(Default)]
struct RequestLimiter(Mutex<Option<SystemTime>>);

impl RequestLimiter {
    fn wait(&self) {
        const MIN_INTERVAL: Duration = Duration::from_secs(2);
        if let Ok(mut last) = self.0.lock() {
            if let Some(previous) = *last {
                let delay =
                    remaining_interval(previous.elapsed().unwrap_or_default(), MIN_INTERVAL);
                if !delay.is_zero() {
                    std::thread::sleep(delay);
                }
            }
            *last = Some(SystemTime::now());
        }
    }
}

fn remaining_interval(elapsed: Duration, interval: Duration) -> Duration {
    interval.saturating_sub(elapsed)
}

fn curl_get(url: &str, max_bytes: usize) -> Result<Option<HttpResponse>, ArtworkError> {
    if !valid_discord_image_url(url) && !url.starts_with("https://xenia-manager.github.io/") {
        return Err(ArtworkError::Provider(
            "rejected non-HTTPS provider URL".to_owned(),
        ));
    }
    let size_limit = max_bytes.to_string();
    let output = Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--max-time",
            "5",
            "--connect-timeout",
            "2",
            "--max-filesize",
            &size_limit,
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--write-out",
            "\n__XERUST_CURL_METADATA__%{http_code}\n%{content_type}\n%{num_redirects}\n%{url_effective}\n",
            url,
        ])
        .output()
        .map_err(|_| ArtworkError::Provider("curl is unavailable".to_owned()))?;
    let Some(mut response) = parse_curl_response(&output.stdout) else {
        if output.status.code() == Some(63) {
            return Err(ArtworkError::Provider(
                "response exceeded configured size limit".to_owned(),
            ));
        }
        return Err(ArtworkError::Provider(
            "HTTPS response metadata was unavailable".to_owned(),
        ));
    };
    response.transfer_ok = output.status.success();
    if output.status.code() == Some(63) || response.body.len() > max_bytes {
        return Err(ArtworkError::Provider(
            "HTTPS response exceeded configured size limit".to_owned(),
        ));
    }
    if response.status != 0 {
        return Ok(Some(response));
    }
    if !output.status.success() || response.status == 0 {
        return Err(ArtworkError::Provider(
            "HTTPS DNS/network request failed".to_owned(),
        ));
    }
    Err(ArtworkError::Provider(
        "HTTPS request returned no HTTP response".to_owned(),
    ))
}

fn parse_curl_response(output: &[u8]) -> Option<HttpResponse> {
    let marker = CURL_METADATA_MARKER.as_bytes();
    let marker_at = output
        .windows(marker.len())
        .rposition(|part| part == marker)?;
    let body = output[..marker_at].to_vec();
    let metadata = std::str::from_utf8(&output[marker_at + marker.len()..]).ok()?;
    let mut lines = metadata.lines();
    let status = lines.next()?.parse().ok()?;
    let content_type = lines.next()?.trim().to_ascii_lowercase();
    let redirects = lines.next()?.parse().ok()?;
    let final_url = lines.next()?.trim().to_owned();
    Some(HttpResponse {
        body,
        status,
        content_type,
        redirects,
        final_url,
        transfer_ok: false,
    })
}

fn is_json_content_type(content_type: &str) -> bool {
    let media_type = content_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    media_type == "application/json" || media_type == "text/json" || media_type.ends_with("+json")
}

fn metadata_response_error(response: &HttpResponse) -> Option<&'static str> {
    if response.status == 0 {
        return Some("no HTTP response (DNS/network failure)");
    }
    if response.status != 200 {
        return Some("HTTP status was not 200");
    }
    if !response.transfer_ok {
        return Some("network transfer failed");
    }
    if response.redirects > MAX_REDIRECTS {
        return Some("redirect limit exceeded");
    }
    if !valid_discord_image_url(&response.final_url) {
        return Some("final URL is not an allowed public HTTPS URL");
    }
    if response.body.len() > MAX_METADATA_BYTES {
        return Some("metadata exceeded size limit");
    }
    if !is_json_content_type(&response.content_type) {
        return Some("response is not JSON MIME");
    }
    None
}

fn is_supported_image_content_type(content_type: &str) -> bool {
    matches!(
        content_type.split(';').next().unwrap_or_default().trim(),
        "image/jpeg" | "image/png" | "image/webp"
    )
}

fn curl_image(url: &str) -> io::Result<HttpResponse> {
    if !valid_discord_image_url(url) {
        return Err(io::Error::other("cover URL is not a valid HTTPS image URL"));
    }
    let output = Command::new("curl")
        .args([
            "--fail",
            "--silent",
            "--show-error",
            "--location",
            "--max-time",
            "5",
            "--connect-timeout",
            "2",
            "--max-filesize",
            "4194304",
            "--proto",
            "=https",
            "--proto-redir",
            "=https",
            "--write-out",
            "\n__XERUST_CURL_METADATA__%{http_code}\n%{content_type}\n%{num_redirects}\n%{url_effective}\n",
            url,
        ])
        .output()?;
    let Some(mut response) = parse_curl_response(&output.stdout) else {
        return Err(io::Error::other("cover response metadata was unavailable"));
    };
    response.transfer_ok = output.status.success();
    if output.status.code() == Some(63) || response.body.len() > MAX_IMAGE_BYTES {
        return Err(io::Error::other(
            "cover response exceeded configured size limit",
        ));
    }
    Ok(response)
}

fn cover_response_error(response: &HttpResponse) -> Option<&'static str> {
    if response.status == 0 {
        return Some("no HTTP response (DNS/network failure)");
    }
    if response.status != 200 {
        return Some("HTTP status was not 200");
    }
    if !response.transfer_ok {
        return Some("curl transfer failed");
    }
    if response.redirects > MAX_REDIRECTS {
        return Some("redirect limit exceeded");
    }
    if !valid_discord_image_url(&response.final_url) {
        return Some("final URL is not an allowed public HTTPS URL");
    }
    if response.body.len() > MAX_IMAGE_BYTES {
        return Some("image exceeded size limit");
    }
    if !is_supported_image_content_type(&response.content_type) {
        return Some("unsupported image MIME type");
    }
    if !is_supported_image(&response.body) {
        return Some("image signature did not match supported formats");
    }
    None
}

fn fetch_and_cache_cover(url: &str, cache_dir: &Path, title_id: &str) -> io::Result<PathBuf> {
    let response = curl_image(url)?;
    if let Some(reason) = cover_response_error(&response) {
        return Err(io::Error::other(format!("cover download failed: {reason}")));
    }
    cache_cover_bytes(&response.body, cache_dir, title_id)
}

fn cache_cover_bytes(bytes: &[u8], cache_dir: &Path, title_id: &str) -> io::Result<PathBuf> {
    if bytes.len() > MAX_IMAGE_BYTES || !is_supported_image(bytes) {
        return Err(io::Error::other("unsupported or oversized image data"));
    }
    fs::create_dir_all(cache_dir)?;
    let path = cache_dir.join(format!("{title_id}.cover"));
    let temporary = cache_dir.join(format!("{title_id}.cover.tmp"));
    fs::write(&temporary, bytes)?;
    #[cfg(windows)]
    if path.exists() {
        fs::remove_file(&path)?;
    }
    fs::rename(temporary, &path)?;
    Ok(path)
}

pub fn valid_discord_image_url(url: &str) -> bool {
    if url.chars().count() > DISCORD_IMAGE_URL_MAX_CHARS || url.chars().any(char::is_whitespace) {
        return false;
    }
    url::Url::parse(url).is_ok_and(|parsed| {
        parsed.scheme() == "https"
            && parsed.host_str().is_some_and(|host| host.contains('.'))
            && parsed.username().is_empty()
            && parsed.password().is_none()
            && !parsed.host_str().is_some_and(|host| {
                host.eq_ignore_ascii_case("localhost")
                    || host.ends_with(".localhost")
                    || host.parse::<std::net::IpAddr>().is_ok_and(|ip| match ip {
                        std::net::IpAddr::V4(ip) => {
                            ip.is_private() || ip.is_loopback() || ip.is_link_local()
                        }
                        std::net::IpAddr::V6(ip) => {
                            ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local()
                        }
                    })
            })
    })
}

fn is_supported_image(bytes: &[u8]) -> bool {
    bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || bytes.starts_with(b"\xff\xd8\xff")
        || bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP")
}

fn read_cache(path: &Path) -> io::Result<CacheEntry> {
    let bytes = fs::read(path)?;
    if bytes.len() > MAX_METADATA_BYTES {
        return Err(io::Error::other("cache file too large"));
    }
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}

fn write_cache(path: &Path, entry: &CacheEntry) -> io::Result<()> {
    fs::create_dir_all(path.parent().unwrap_or_else(|| Path::new(".")))?;
    let bytes = serde_json::to_vec(entry).map_err(io::Error::other)?;
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, bytes)?;
    #[cfg(windows)]
    if path.exists() {
        fs::remove_file(path)?;
    }
    fs::rename(temporary, path)
}

fn prune_cache(path: &Path) {
    let Ok(entries) = fs::read_dir(path) else {
        return;
    };
    let mut files = entries
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension == "json")
        })
        .filter_map(|entry| {
            Some((
                entry.metadata().ok()?.modified().ok()?,
                entry.metadata().ok()?.len(),
                entry.path(),
            ))
        })
        .collect::<Vec<_>>();
    let mut total = files.iter().map(|(_, bytes, _)| bytes).sum::<u64>();
    for (_, _bytes, file) in &files {
        if let Some(stem) = file.file_stem().and_then(|name| name.to_str()) {
            let cover_bytes = fs::metadata(path.join(format!("{stem}.cover")))
                .map(|metadata| metadata.len())
                .unwrap_or_default();
            total = total.saturating_add(cover_bytes);
        }
    }
    if files.len() <= MAX_CACHE_ENTRIES && total <= MAX_CACHE_BYTES {
        return;
    }
    files.sort_by_key(|(modified, _, _)| *modified);
    for (_, bytes, file) in files {
        if files_len_under_limit(path) <= MAX_CACHE_ENTRIES && total <= MAX_CACHE_BYTES {
            break;
        }
        total = total.saturating_sub(bytes);
        if let Some(stem) = file.file_stem().and_then(|name| name.to_str()) {
            let cover = path.join(format!("{stem}.cover"));
            total = total.saturating_sub(
                fs::metadata(&cover)
                    .map(|metadata| metadata.len())
                    .unwrap_or_default(),
            );
            let _ = fs::remove_file(cover);
        }
        let _ = fs::remove_file(file);
    }
}

fn files_len_under_limit(path: &Path) -> usize {
    fs::read_dir(path)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|entry| {
                    entry
                        .path()
                        .extension()
                        .is_some_and(|extension| extension == "json")
                })
                .count()
        })
        .unwrap_or_default()
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        collections::HashMap,
        sync::atomic::{AtomicUsize, Ordering},
    };

    static TEMP_ID: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn curl_transport_parses_redirect_and_content_metadata_separately_from_body() {
        let bytes = [
            b"\x89PNG\r\n\x1a\n".as_slice(),
            CURL_METADATA_MARKER.as_bytes(),
            b"200\nimage/png; charset=binary\n2\nhttps://cdn.example/game.png\n",
        ]
        .concat();
        let response = parse_curl_response(&bytes).unwrap();
        assert_eq!(response.body, b"\x89PNG\r\n\x1a\n");
        assert_eq!(response.status, 200);
        assert_eq!(response.redirects, 2);
        assert_eq!(response.final_url, "https://cdn.example/game.png");
        assert!(is_supported_image_content_type(&response.content_type));
        assert!(!is_supported_image(&b"<html>error</html>"[..]));
    }

    #[test]
    fn provider_json_content_type_is_checked_with_parameters() {
        assert!(is_json_content_type("application/json; charset=utf-8"));
        assert!(is_json_content_type("application/vnd.example+json"));
        assert!(!is_json_content_type("text/html"));
        assert!(!is_supported_image_content_type("text/html"));
    }

    #[test]
    fn metadata_and_cover_transport_checks_reject_bad_status_redirect_mime_and_signature() {
        let valid = HttpResponse {
            body: b"\x89PNG\r\n\x1a\n".to_vec(),
            status: 200,
            content_type: "image/png".to_owned(),
            redirects: 1,
            final_url: "https://cdn.example/game.png".to_owned(),
            transfer_ok: true,
        };
        assert_eq!(cover_response_error(&valid), None);
        let mut invalid = valid;
        invalid.redirects = MAX_REDIRECTS + 1;
        assert_eq!(
            cover_response_error(&invalid),
            Some("redirect limit exceeded")
        );
        invalid.redirects = 0;
        invalid.content_type = "text/html".to_owned();
        assert_eq!(
            cover_response_error(&invalid),
            Some("unsupported image MIME type")
        );
        invalid.content_type = "image/png".to_owned();
        invalid.body = b"<html>not an image</html>".to_vec();
        assert_eq!(
            cover_response_error(&invalid),
            Some("image signature did not match supported formats")
        );

        let metadata = HttpResponse {
            body: b"[]".to_vec(),
            status: 200,
            content_type: "application/json".to_owned(),
            redirects: 0,
            final_url: "https://metadata.example/games.json".to_owned(),
            transfer_ok: true,
        };
        assert_eq!(metadata_response_error(&metadata), None);
        let mut bad_metadata = metadata;
        bad_metadata.final_url = "http://metadata.example/games.json".to_owned();
        assert_eq!(
            metadata_response_error(&bad_metadata),
            Some("final URL is not an allowed public HTTPS URL")
        );

        let mut http_error = bad_metadata;
        http_error.status = 503;
        http_error.transfer_ok = false;
        assert_eq!(
            metadata_response_error(&http_error),
            Some("HTTP status was not 200")
        );
        assert_eq!(
            cover_response_error(&http_error),
            Some("HTTP status was not 200")
        );
        http_error.status = 0;
        assert_eq!(
            metadata_response_error(&http_error),
            Some("no HTTP response (DNS/network failure)")
        );
    }

    struct FixtureProvider {
        name: &'static str,
        items: HashMap<String, GameMetadata>,
        calls: AtomicUsize,
        fail: bool,
    }

    impl FixtureProvider {
        fn new(name: &'static str, values: &[(&str, &str, Option<&str>, Option<bool>)]) -> Self {
            Self {
                name,
                items: values
                    .iter()
                    .map(|(id, title, url, official)| {
                        let id = crate::catalog::normalize_title_id(id).unwrap();
                        (
                            id.clone(),
                            GameMetadata {
                                title_id: id,
                                name: Some((*title).to_owned()),
                                cover_url: url.map(str::to_owned),
                                source: name.to_owned(),
                                official_cover: *official,
                                related_title_ids: Vec::new(),
                                cached_cover_path: None,
                            },
                        )
                    })
                    .collect(),
                calls: AtomicUsize::new(0),
                fail: false,
            }
        }
    }

    impl MetadataProvider for FixtureProvider {
        fn name(&self) -> &'static str {
            self.name
        }
        fn lookup(&self, title_id: &str) -> Result<Option<GameMetadata>, ArtworkError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.fail {
                return Err(ArtworkError::Provider(self.name.to_owned()));
            }
            Ok(self
                .items
                .values()
                .find(|metadata| {
                    metadata.title_id == title_id
                        || metadata.related_title_ids.iter().any(|id| id == title_id)
                })
                .cloned())
        }
    }

    fn temp_dir() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "xerust-artwork-test-{}-{}-{}",
            std::process::id(),
            unix_now(),
            TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn fixture_ids_resolve_and_unknown_ids_are_not_guessed() {
        let mut provider = Arc::new(FixtureProvider::new(
            "fixture",
            &[
                (
                    "4D5307E6",
                    "Halo 3",
                    Some("https://art.example/halo3.jpg"),
                    Some(true),
                ),
                (
                    "4D53085B",
                    "Halo: Reach",
                    Some("https://art.example/reach.jpg"),
                    Some(true),
                ),
                (
                    "584111F7",
                    "Minecraft: Xbox 360 Edition",
                    Some("https://art.example/minecraft.jpg"),
                    None,
                ),
            ],
        ));
        Arc::get_mut(&mut provider)
            .unwrap()
            .items
            .get_mut("4D5307E6")
            .unwrap()
            .related_title_ids
            .push("4D5307E7".to_owned());
        let resolver = ArtworkResolver::new(temp_dir(), vec![provider], true);
        assert!(resolver.resolve("DEADBEEF").unwrap().is_none());
        let game = resolver.resolve("0x4d5307e6").unwrap().unwrap();
        assert_eq!(game.name.as_deref(), Some("Halo 3"));
        assert_eq!(
            game.cover_url.as_deref(),
            Some("https://art.example/halo3.jpg")
        );
        assert_eq!(
            resolver
                .resolve("4D53085B")
                .unwrap()
                .unwrap()
                .name
                .as_deref(),
            Some("Halo: Reach")
        );
        assert_eq!(
            resolver
                .resolve("4D5307E7")
                .unwrap()
                .unwrap()
                .name
                .as_deref(),
            Some("Halo 3")
        );
        let xbla = resolver.resolve("584111F7").unwrap().unwrap();
        assert_eq!(xbla.name.as_deref(), Some("Minecraft: Xbox 360 Edition"));
        assert_eq!(
            xbla.cover_url.as_deref(),
            Some("https://art.example/minecraft.jpg")
        );
    }

    #[test]
    fn x360db_index_schema_maps_canonical_and_alternate_ids() {
        let index: Vec<X360DbEntry> = serde_json::from_str(
            r#"[
                {
                    "id":"4D5307E6",
                    "alternative_id":["4D5307E7"],
                    "title":"Halo 3",
                    "boxart":"http://download.xbox.com/halo3.jpg"
                }
            ]"#,
        )
        .unwrap();
        let canonical = find_x360db_title(&index, "4D5307E6").unwrap();
        let alternate = find_x360db_title(&index, "4D5307E7").unwrap();
        assert_eq!(canonical.name.as_deref(), Some("Halo 3"));
        assert_eq!(
            canonical.cover_url.as_deref(),
            Some("https://xenia-manager.github.io/x360db/titles/4D5307E6/artwork/boxart.jpg")
        );
        assert_eq!(alternate.title_id, "4D5307E6");
        assert!(alternate.related_title_ids.contains(&"4D5307E7".to_owned()));
        assert!(find_x360db_title(&index, "DEADBEEF").is_none());
    }

    #[test]
    fn provider_fallback_merges_metadata_and_rejects_bad_urls() {
        let primary = Arc::new(FixtureProvider::new(
            "primary",
            &[(
                "4D5307E6",
                "Halo 3",
                Some("https://art.example/community.jpg"),
                None,
            )],
        ));
        let fallback = Arc::new(FixtureProvider::new(
            "fallback",
            &[(
                "4D5307E6",
                "Halo 3",
                Some("https://art.example/halo3.jpg"),
                Some(true),
            )],
        ));
        let resolver =
            ArtworkResolver::new(temp_dir(), vec![primary.clone(), fallback.clone()], true);
        let game = resolver.resolve("4D5307E6").unwrap().unwrap();
        assert_eq!(game.name.as_deref(), Some("Halo 3"));
        assert_eq!(
            game.cover_url.as_deref(),
            Some("https://art.example/halo3.jpg")
        );
        assert_eq!(game.official_cover, Some(true));
        assert_eq!(fallback.calls.load(Ordering::SeqCst), 1);
        assert!(!valid_discord_image_url("http://art.example/cover.jpg"));
        assert!(!valid_discord_image_url("https://art.example/a b.jpg"));
        assert!(!valid_discord_image_url("https://127.0.0.1/cover.jpg"));
        assert!(!valid_discord_image_url("https://localhost/cover.jpg"));
    }

    #[test]
    fn cache_serves_offline_data_and_prevents_repeated_provider_queries() {
        let path = temp_dir();
        let provider = Arc::new(FixtureProvider::new(
            "fixture",
            &[(
                "4D5307E6",
                "Halo 3",
                Some("https://art.example/halo3.jpg"),
                Some(true),
            )],
        ));
        let online = ArtworkResolver::new(path.clone(), vec![provider.clone()], true);
        assert!(online.resolve("4D5307E6").unwrap().is_some());
        assert!(online.resolve("4D5307E6").unwrap().is_some());
        assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
        let offline = ArtworkResolver::new(path, Vec::new(), false);
        assert_eq!(
            offline
                .resolve("4D5307E6")
                .unwrap()
                .unwrap()
                .name
                .as_deref(),
            Some("Halo 3")
        );
        assert!(offline.resolve("DEADBEEF").unwrap().is_none());
    }

    #[test]
    fn provider_failure_does_not_create_a_fabricated_game() {
        let mut failing = FixtureProvider::new("failed", &[]);
        failing.fail = true;
        let resolver = ArtworkResolver::new(temp_dir(), vec![Arc::new(failing)], true);
        assert!(resolver.resolve("DEADBEEF").is_err());
        assert!(resolver.resolve("not-a-title").is_err());
    }

    #[test]
    fn provider_failure_uses_a_stale_exact_id_cache_when_available() {
        let directory = temp_dir();
        let provider = Arc::new(FixtureProvider::new(
            "fixture",
            &[(
                "4D5307E6",
                "Halo 3",
                Some("https://art.example/halo3.jpg"),
                Some(true),
            )],
        ));
        let resolver = ArtworkResolver::new(directory.clone(), vec![provider], true);
        resolver.resolve("4D5307E6").unwrap();
        let cache_path = directory.join("4D5307E6.json");
        let mut entry = read_cache(&cache_path).unwrap();
        entry.checked_at = 0;
        write_cache(&cache_path, &entry).unwrap();

        let mut failing = FixtureProvider::new("offline", &[]);
        failing.fail = true;
        let resolver = ArtworkResolver::new(directory, vec![Arc::new(failing)], true);
        assert_eq!(
            resolver
                .resolve("4D5307E6")
                .unwrap()
                .unwrap()
                .name
                .as_deref(),
            Some("Halo 3")
        );
    }

    #[test]
    fn image_cache_accepts_bounded_png_jpeg_and_rejects_unknown_formats() {
        let directory = temp_dir();
        let png = b"\x89PNG\r\n\x1a\nfixture";
        let path = cache_cover_bytes(png, &directory, "4D5307E6").unwrap();
        assert_eq!(fs::read(path).unwrap(), png);
        assert!(cache_cover_bytes(b"not an image", &directory, "4D53085B").is_err());
        assert!(cache_cover_bytes(&vec![0; MAX_IMAGE_BYTES + 1], &directory, "584111F7").is_err());
    }

    #[test]
    fn xboxunity_fixture_prefers_explicitly_official_cover() {
        let bytes = br#"{
            "Covers": [
                {"CoverID":"101", "Official":"0"},
                {"CoverID":"202", "Official":"1"}
            ],
            "CoversCount": 2
        }"#;
        let metadata = parse_xboxunity_covers(bytes, "4D5307E6").unwrap().unwrap();
        assert_eq!(metadata.title_id, "4D5307E6");
        assert_eq!(metadata.official_cover, Some(true));
        assert_eq!(
            metadata.cover_url.as_deref(),
            Some("https://xboxunity.net/Resources/Lib/Cover.php?cid=202&size=large")
        );
    }

    #[test]
    fn request_rate_interval_is_enforced_without_overwaiting() {
        assert_eq!(
            remaining_interval(Duration::from_millis(500), Duration::from_secs(2)),
            Duration::from_millis(1500)
        );
        assert_eq!(
            remaining_interval(Duration::from_secs(3), Duration::from_secs(2)),
            Duration::ZERO
        );
    }
}
