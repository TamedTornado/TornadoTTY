use std::collections::{BTreeMap, BTreeSet};
use std::net::{Ipv4Addr, Ipv6Addr};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerUrlCandidate {
    pub url: String,
    pub origin: String,
    pub display: String,
    pub port: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServerUrlError {
    Empty,
    Invalid,
    UnsupportedScheme,
    UnsupportedHost,
    MissingPort,
    InvalidPort,
}

/// Normalizes one source-compatible local development-server URL.
///
/// # Errors
///
/// Rejects malformed input, unsupported schemes or hosts, and absent/invalid
/// ports.
pub fn normalize_server_url(raw: &str) -> Result<ServerUrlCandidate, ServerUrlError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err(ServerUrlError::Empty);
    }
    let candidate = if raw.bytes().all(|byte| byte.is_ascii_digit()) {
        format!("http://localhost:{raw}")
    } else if raw.contains("://") {
        raw.to_owned()
    } else {
        format!("http://{raw}")
    };
    let (scheme, remainder) = candidate.split_once("://").ok_or(ServerUrlError::Invalid)?;
    let scheme = scheme.to_ascii_lowercase();
    if scheme != "http" && scheme != "https" {
        return Err(ServerUrlError::UnsupportedScheme);
    }
    let boundary = remainder.find(['/', '?', '#']).unwrap_or(remainder.len());
    let authority = &remainder[..boundary];
    let suffix = &remainder[boundary..];
    if authority.contains('@') {
        return Err(ServerUrlError::Invalid);
    }
    let (raw_host, raw_port) = split_host_port(authority)?;
    let host = normalize_host(raw_host)?;
    let port = raw_port
        .parse::<u16>()
        .map_err(|_| ServerUrlError::InvalidPort)?;
    if port == 0 {
        return Err(ServerUrlError::InvalidPort);
    }
    let formatted_host = if host.contains(':') {
        format!("[{host}]")
    } else {
        host.clone()
    };
    let origin = format!("{scheme}://{formatted_host}:{port}");
    let url = format!("{origin}{}", if suffix.is_empty() { "/" } else { suffix });
    Ok(ServerUrlCandidate {
        url,
        origin,
        display: format!("{formatted_host}:{port}"),
        port,
    })
}

fn split_host_port(authority: &str) -> Result<(&str, &str), ServerUrlError> {
    if let Some(host) = authority.strip_prefix('[') {
        let end = host.find(']').ok_or(ServerUrlError::Invalid)?;
        let raw_host = &host[..end];
        let tail = &host[end + 1..];
        let port = tail.strip_prefix(':').ok_or(ServerUrlError::MissingPort)?;
        if port.is_empty() {
            return Err(ServerUrlError::MissingPort);
        }
        return Ok((raw_host, port));
    }
    let (host, port) = authority
        .rsplit_once(':')
        .ok_or(ServerUrlError::MissingPort)?;
    if host.is_empty() || port.is_empty() {
        return Err(ServerUrlError::MissingPort);
    }
    Ok((host, port))
}

fn normalize_host(raw: &str) -> Result<String, ServerUrlError> {
    let host = raw.trim_matches(['[', ']']).to_ascii_lowercase();
    if matches!(
        host.as_str(),
        "localhost" | "0.0.0.0" | "127.0.0.1" | "::" | "::1"
    ) {
        return Ok("localhost".into());
    }
    if host.strip_suffix(".local").is_some() {
        return Ok(host);
    }
    if let Ok(address) = host.parse::<Ipv4Addr>() {
        if address.is_loopback() || address.is_private() || address.is_link_local() {
            return Ok(host);
        }
        return Err(ServerUrlError::UnsupportedHost);
    }
    if let Ok(address) = host.parse::<Ipv6Addr>() {
        let first = address.segments()[0];
        if (first & 0xfe00) == 0xfc00 || (first & 0xffc0) == 0xfe80 {
            return Ok(host);
        }
    }
    Err(ServerUrlError::UnsupportedHost)
}

#[must_use]
pub fn detect_server_urls(text: &str) -> Vec<ServerUrlCandidate> {
    let mut candidates = Vec::new();
    let mut remaining = text;
    while let Some((offset, _)) = ["http://", "https://"]
        .iter()
        .filter_map(|prefix| remaining.find(prefix).map(|offset| (offset, prefix)))
        .min_by_key(|(offset, _)| *offset)
    {
        let start = &remaining[offset..];
        let end = start
            .find(|character: char| character.is_whitespace() || "<>\"'".contains(character))
            .unwrap_or(start.len());
        let raw = start[..end].trim_end_matches(['.', ',', ';', ':', ')', ']', '}']);
        if let Ok(candidate) = normalize_server_url(raw)
            && !candidates
                .iter()
                .any(|existing: &ServerUrlCandidate| existing.origin == candidate.origin)
        {
            candidates.push(candidate);
        }
        remaining = &start[end..];
    }
    candidates.sort_by(|left, right| {
        left.port
            .cmp(&right.port)
            .then_with(|| host_rank(left).cmp(&host_rank(right)))
            .then_with(|| left.origin.cmp(&right.origin))
    });
    candidates
}

fn host_rank(candidate: &ServerUrlCandidate) -> u8 {
    if candidate.origin.contains("://localhost:") {
        0
    } else if candidate.origin.contains("://127.") {
        1
    } else {
        2
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ServerPortRule {
    lower: u16,
    upper: u16,
}

impl ServerPortRule {
    pub fn parse(raw: &str) -> Option<Self> {
        let components = raw.trim().split('-').map(str::trim).collect::<Vec<_>>();
        let (lower, upper) = match components.as_slice() {
            [port] => {
                let port = valid_port(port)?;
                (port, port)
            }
            [lower, upper] => (valid_port(lower)?, valid_port(upper)?),
            _ => return None,
        };
        (lower <= upper).then_some(Self { lower, upper })
    }

    #[must_use]
    pub fn canonical(&self) -> String {
        if self.lower == self.upper {
            self.lower.to_string()
        } else {
            format!("{}-{}", self.lower, self.upper)
        }
    }

    #[must_use]
    pub fn contains(self, port: u16) -> bool {
        (self.lower..=self.upper).contains(&port)
    }

    pub fn normalize<I, S>(rules: I) -> Vec<Self>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut parsed = rules
            .into_iter()
            .filter_map(|rule| Self::parse(rule.as_ref()))
            .collect::<Vec<_>>();
        parsed.sort_unstable();
        let mut merged: Vec<Self> = Vec::new();
        for rule in parsed {
            if let Some(last) = merged.last_mut()
                && u32::from(rule.lower) <= u32::from(last.upper) + 1
            {
                last.upper = last.upper.max(rule.upper);
                continue;
            }
            merged.push(rule);
        }
        merged
    }

    pub fn adding_port(port: u16, rules: &[&str]) -> Vec<String> {
        let mut values = rules.iter().map(ToString::to_string).collect::<Vec<_>>();
        values.push(port.to_string());
        Self::normalize(&values)
            .into_iter()
            .map(|rule| rule.canonical())
            .collect()
    }

    #[must_use]
    pub fn removing_port(port: u16, rules: &[&str]) -> Vec<String> {
        let mut result = Vec::new();
        for rule in Self::normalize(rules) {
            if rule.contains(port) {
                if port > rule.lower {
                    result.push(Self {
                        lower: rule.lower,
                        upper: port - 1,
                    });
                }
                if port < rule.upper {
                    result.push(Self {
                        lower: port + 1,
                        upper: rule.upper,
                    });
                }
            } else {
                result.push(rule);
            }
        }
        result.into_iter().map(|rule| rule.canonical()).collect()
    }
}

fn valid_port(raw: &str) -> Option<u16> {
    let port = raw.parse::<u16>().ok()?;
    (port > 0).then_some(port)
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DetectedServerSource {
    Manual,
    Watch,
    Docker,
    Scanner,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DetectedServerConfidence {
    Explicit,
    Pid,
    Cwd,
    Worklane,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DetectedServer {
    pub id: String,
    pub origin: String,
    pub url: String,
    pub display: String,
    pub worklane_id: String,
    pub pane_id: Option<String>,
    pub source: DetectedServerSource,
    pub ports: Vec<u16>,
    pub confidence: DetectedServerConfidence,
    pub updated_at_ms: u64,
    pub first_seen_at_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerTerminationObservation {
    pub pane_id: Option<String>,
    pub listener_pid: u32,
    pub listener_start_time: u64,
    pub port: u16,
    pub owned_by_pane: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServerTerminationTarget {
    pub pid: u32,
    pub start_time: u64,
}

#[must_use]
pub fn authorize_server_termination(
    server: &DetectedServer,
    pane_id: &str,
    pane_shell_pid: u32,
    observation: &ServerTerminationObservation,
) -> Option<ServerTerminationTarget> {
    (server.source == DetectedServerSource::Scanner
        && server.confidence == DetectedServerConfidence::Pid
        && server.pane_id.as_deref() == Some(pane_id)
        && observation.pane_id.as_deref() == Some(pane_id)
        && observation.owned_by_pane
        && pane_shell_pid > 1
        && observation.listener_pid > 1
        && server.ports.contains(&observation.port))
    .then_some(ServerTerminationTarget {
        pid: observation.listener_pid,
        start_time: observation.listener_start_time,
    })
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ServerRegistry {
    records: BTreeMap<ServerRecordKey, DetectedServer>,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ServerRecordKey {
    worklane_id: String,
    origin: String,
    source: DetectedServerSource,
    pane_id: Option<String>,
}

impl ServerRegistry {
    pub fn upsert(&mut self, mut server: DetectedServer) {
        let key = ServerRecordKey::from(&server);
        if let Some(previous) = self.records.get(&key) {
            server.first_seen_at_ms = server.first_seen_at_ms.min(previous.first_seen_at_ms);
        }
        self.records.insert(key, server);
    }

    pub fn replace_source(
        &mut self,
        source: DetectedServerSource,
        worklane_id: &str,
        servers: Vec<DetectedServer>,
    ) {
        let previous = self.records.clone();
        self.records
            .retain(|key, _| key.worklane_id != worklane_id || key.source != source);
        for mut server in servers {
            let key = ServerRecordKey::from(&server);
            if let Some(old) = previous.get(&key) {
                server.first_seen_at_ms = server.first_seen_at_ms.min(old.first_seen_at_ms);
            }
            self.records.insert(key, server);
        }
    }

    pub fn clear_pane(
        &mut self,
        worklane_id: &str,
        pane_id: &str,
        source: Option<DetectedServerSource>,
    ) {
        self.records.retain(|key, _| {
            key.worklane_id != worklane_id
                || key.pane_id.as_deref() != Some(pane_id)
                || source.is_some_and(|source| key.source != source)
        });
    }

    #[must_use]
    pub fn servers_in(&self, worklane_id: &str) -> Vec<DetectedServer> {
        let mut grouped: BTreeMap<&str, Vec<&DetectedServer>> = BTreeMap::new();
        for server in self
            .records
            .values()
            .filter(|server| server.worklane_id == worklane_id)
        {
            grouped.entry(&server.origin).or_default().push(server);
        }
        let mut merged = grouped
            .into_values()
            .filter_map(|records| merge_server_records(&records))
            .collect::<Vec<_>>();
        merged.sort_by(|left, right| {
            right
                .updated_at_ms
                .cmp(&left.updated_at_ms)
                .then_with(|| left.origin.cmp(&right.origin))
        });
        merged
    }
}

impl From<&DetectedServer> for ServerRecordKey {
    fn from(server: &DetectedServer) -> Self {
        Self {
            worklane_id: server.worklane_id.clone(),
            origin: server.origin.clone(),
            source: server.source,
            pane_id: server.pane_id.clone(),
        }
    }
}

fn merge_server_records(records: &[&DetectedServer]) -> Option<DetectedServer> {
    let winner = records.iter().copied().max_by(|left, right| {
        source_priority(left.source)
            .cmp(&source_priority(right.source))
            .then_with(|| left.updated_at_ms.cmp(&right.updated_at_ms))
            .then_with(|| right.origin.cmp(&left.origin))
    })?;
    let mut merged = winner.clone();
    merged.id = format!("{}|{}", winner.worklane_id, winner.origin);
    merged.ports = records
        .iter()
        .flat_map(|server| server.ports.iter().copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    merged.first_seen_at_ms = records
        .iter()
        .map(|server| server.first_seen_at_ms)
        .min()
        .unwrap_or(winner.first_seen_at_ms);
    Some(merged)
}

const fn source_priority(source: DetectedServerSource) -> u8 {
    match source {
        DetectedServerSource::Manual => 4,
        DetectedServerSource::Watch => 3,
        DetectedServerSource::Docker => 2,
        DetectedServerSource::Scanner => 1,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServerRelevanceTier {
    Primary,
    Shown,
    Hidden,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ServerRelevanceReason {
    Selected,
    IgnoredPort(u16),
    Manual,
    RunningPane,
    FocusedPane,
    Source(DetectedServerSource),
    Confidence(DetectedServerConfidence),
    Fresh,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RankedServer {
    pub server: DetectedServer,
    pub tier: ServerRelevanceTier,
    pub score: i32,
    pub reasons: BTreeSet<ServerRelevanceReason>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ServerRelevanceContext {
    pub focused_pane_id: Option<String>,
    pub running_pane_ids: BTreeSet<String>,
    pub ignored_port_rules: Vec<ServerPortRule>,
    pub selected_origin: Option<String>,
    pub now_ms: u64,
}

#[must_use]
pub fn rank_servers(
    servers: Vec<DetectedServer>,
    context: &ServerRelevanceContext,
) -> Vec<RankedServer> {
    let mut visible = Vec::new();
    let mut hidden = Vec::new();
    for server in servers {
        let port = server.ports.first().copied();
        if server.source != DetectedServerSource::Manual
            && let Some(port) = port
            && context
                .ignored_port_rules
                .iter()
                .any(|rule| rule.contains(port))
        {
            hidden.push(RankedServer {
                server,
                tier: ServerRelevanceTier::Hidden,
                score: 0,
                reasons: BTreeSet::from([ServerRelevanceReason::IgnoredPort(port)]),
            });
            continue;
        }
        visible.push(score_server(server, context));
    }
    visible.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| left.server.origin.cmp(&right.server.origin))
    });
    for (index, entry) in visible.iter_mut().enumerate() {
        entry.tier = if index == 0 {
            ServerRelevanceTier::Primary
        } else {
            ServerRelevanceTier::Shown
        };
    }
    visible.extend(hidden);
    visible
}

fn score_server(server: DetectedServer, context: &ServerRelevanceContext) -> RankedServer {
    let mut score = 0;
    let mut reasons = BTreeSet::new();
    if context.selected_origin.as_ref() == Some(&server.origin) {
        score += 1000;
        reasons.insert(ServerRelevanceReason::Selected);
    }
    if server.pane_id.as_ref() == context.focused_pane_id.as_ref() && server.pane_id.is_some() {
        score += 200;
        reasons.insert(ServerRelevanceReason::FocusedPane);
    }
    if server
        .pane_id
        .as_ref()
        .is_some_and(|pane| context.running_pane_ids.contains(pane))
    {
        score += 150;
        reasons.insert(ServerRelevanceReason::RunningPane);
    }
    score += match server.source {
        DetectedServerSource::Manual => 80,
        DetectedServerSource::Watch => 60,
        DetectedServerSource::Docker => 40,
        DetectedServerSource::Scanner => 0,
    };
    if server.source == DetectedServerSource::Manual {
        reasons.insert(ServerRelevanceReason::Manual);
    }
    reasons.insert(ServerRelevanceReason::Source(server.source));
    score += match server.confidence {
        DetectedServerConfidence::Explicit => 30,
        DetectedServerConfidence::Pid => 20,
        DetectedServerConfidence::Cwd => 10,
        DetectedServerConfidence::Worklane => 0,
    };
    reasons.insert(ServerRelevanceReason::Confidence(server.confidence));
    if context.now_ms.saturating_sub(server.first_seen_at_ms) <= 60_000 {
        score += 5;
        reasons.insert(ServerRelevanceReason::Fresh);
    }
    RankedServer {
        server,
        tier: ServerRelevanceTier::Shown,
        score,
        reasons,
    }
}
