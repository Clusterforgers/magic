//! Server settings: six come from the environment, the rest are limits that tests may tune.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use thiserror::Error;

use crate::actor::GameConfig;
use crate::pace::MAX_CLOCK_SECS;

/// Listen address when `MTG_BIND` is unset.
pub const DEFAULT_BIND: SocketAddr = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8080);
/// Cap on concurrent games when `MTG_MAX_GAMES` is unset.
pub const DEFAULT_MAX_GAMES: usize = 100;

/// Why the environment does not describe a usable server.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ConfigError {
    /// `MTG_BIND` is not an `ip:port` pair.
    #[error("MTG_BIND must be an ip:port such as 127.0.0.1:8080, got {0:?}")]
    BadBind(String),
    /// `MTG_MAX_GAMES` is not a whole number of at least 1.
    #[error("MTG_MAX_GAMES must be a whole number of at least 1, got {0:?}")]
    BadMaxGames(String),
    /// An `MTG_ALLOWED_ORIGINS` entry is not `http(s)://host[:port]`.
    #[error("MTG_ALLOWED_ORIGINS entry {0:?} is not an origin such as https://host[:port]")]
    BadOrigin(String),
    /// `MTG_AUTO_PASS` is not a switch.
    #[error("MTG_AUTO_PASS must be 1 or 0 (also on, off, true, false), got {0:?}")]
    BadAutoPass(String),
    /// `MTG_RESPONSE_SECS` is not a number of seconds.
    #[error(
        "MTG_RESPONSE_SECS must be a whole number of seconds, 0 for no clock, at most 86400, got {0:?}"
    )]
    BadResponseSecs(String),
    /// `MTG_BLOCK_SECS` is not a number of seconds.
    #[error(
        "MTG_BLOCK_SECS must be a whole number of seconds, 0 for no clock, at most 86400, got {0:?}"
    )]
    BadBlockSecs(String),
}

/// Timers and size limits of one WebSocket connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SocketLimits {
    /// Time a new socket has to send `Join`.
    pub join_timeout: Duration,
    /// Time between pings.
    pub ping_every: Duration,
    /// A socket that answered no ping for this long is closed.
    pub idle_timeout: Duration,
    /// Time one outgoing frame may take before the client counts as stalled.
    pub send_timeout: Duration,
    /// Largest incoming frame and message, in bytes.
    pub max_frame: usize,
    /// Messages a connection may send in a burst.
    pub burst: u32,
    /// Messages per second a connection may keep sending.
    pub per_second: u32,
    /// Malformed messages in a row after which the socket is closed.
    pub max_bad_in_a_row: u32,
}

impl Default for SocketLimits {
    fn default() -> Self {
        SocketLimits {
            join_timeout: Duration::from_secs(10),
            ping_every: Duration::from_secs(20),
            idle_timeout: Duration::from_secs(60),
            send_timeout: Duration::from_secs(10),
            max_frame: 64 * 1024,
            burst: 40,
            per_second: 20,
            max_bad_in_a_row: 5,
        }
    }
}

/// How long the registry keeps games nobody is using.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Retention {
    /// Time between sweeps.
    pub sweep_every: Duration,
    /// A finished game is dropped this long after the sweep first saw it finished.
    pub finished_after: Duration,
    /// A lobby that never started is dropped this long after creation.
    pub lobby_after: Duration,
    /// A started game without any connected seat is dropped after this long.
    pub abandoned_after: Duration,
}

impl Default for Retention {
    fn default() -> Self {
        Retention {
            sweep_every: Duration::from_secs(30),
            finished_after: Duration::from_mins(10),
            lobby_after: Duration::from_hours(1),
            abandoned_after: Duration::from_hours(24),
        }
    }
}

/// Everything the server needs to start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerConfig {
    /// Address to listen on (`MTG_BIND`).
    pub bind: SocketAddr,
    /// Origins besides the server's own host that may open a game socket (`MTG_ALLOWED_ORIGINS`).
    pub allowed_origins: Vec<String>,
    /// Most games alive at once (`MTG_MAX_GAMES`).
    pub max_games: usize,
    /// Seed every game uses instead of a random one. For tests only: never read from the environment.
    pub fixed_seed: Option<u64>,
    /// Tunables handed to each game actor.
    pub game: GameConfig,
    /// Per-connection timers and limits.
    pub socket: SocketLimits,
    /// When the janitor drops games.
    pub retention: Retention,
    /// How long shutdown waits for sockets to finish closing.
    pub shutdown_grace: Duration,
}

impl Default for ServerConfig {
    fn default() -> Self {
        ServerConfig {
            bind: DEFAULT_BIND,
            allowed_origins: Vec::new(),
            max_games: DEFAULT_MAX_GAMES,
            fixed_seed: None,
            game: GameConfig::default(),
            socket: SocketLimits::default(),
            retention: Retention::default(),
            shutdown_grace: Duration::from_secs(5),
        }
    }
}

impl ServerConfig {
    /// Reads `MTG_BIND`, `MTG_ALLOWED_ORIGINS`, `MTG_MAX_GAMES`, `MTG_AUTO_PASS`, `MTG_RESPONSE_SECS` and
    /// `MTG_BLOCK_SECS` from the process environment.
    pub fn from_env() -> Result<ServerConfig, ConfigError> {
        ServerConfig::from_lookup(|key| {
            std::env::var_os(key).map(|value| value.to_string_lossy().into_owned())
        })
    }

    /// Like [`ServerConfig::from_env`] with variables supplied by `lookup`; blank values count as unset.
    pub fn from_lookup(
        lookup: impl Fn(&str) -> Option<String>,
    ) -> Result<ServerConfig, ConfigError> {
        let get = |key: &str| lookup(key).filter(|value| !value.trim().is_empty());
        let mut config = ServerConfig::default();
        if let Some(text) = get("MTG_BIND") {
            config.bind = text
                .trim()
                .parse()
                .map_err(|_| ConfigError::BadBind(text.clone()))?;
        }
        if let Some(text) = get("MTG_MAX_GAMES") {
            config.max_games = match text.trim().parse::<usize>() {
                Ok(n) if n >= 1 => n,
                _ => return Err(ConfigError::BadMaxGames(text)),
            };
        }
        if let Some(text) = get("MTG_ALLOWED_ORIGINS") {
            for entry in text.split(',').map(str::trim).filter(|e| !e.is_empty()) {
                let origin = normalize_origin(entry)
                    .ok_or_else(|| ConfigError::BadOrigin(entry.to_owned()))?;
                if !config.allowed_origins.contains(&origin) {
                    config.allowed_origins.push(origin);
                }
            }
        }
        if let Some(text) = get("MTG_AUTO_PASS") {
            config.game.pace.auto_pass =
                parse_switch(&text).ok_or(ConfigError::BadAutoPass(text))?;
        }
        if let Some(text) = get("MTG_RESPONSE_SECS") {
            let seconds = parse_seconds(&text).ok_or(ConfigError::BadResponseSecs(text))?;
            config.game.pace.response = clock_of(seconds);
        }
        if let Some(text) = get("MTG_BLOCK_SECS") {
            let seconds = parse_seconds(&text).ok_or(ConfigError::BadBlockSecs(text))?;
            config.game.pace.blocks = clock_of(seconds);
        }
        Ok(config)
    }
}

/// A switch written as 1/0, on/off or true/false, in any case.
fn parse_switch(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "on" | "true" => Some(true),
        "0" | "off" | "false" => Some(false),
        _ => None,
    }
}

/// A clock in whole seconds; nothing above a day is accepted.
fn parse_seconds(raw: &str) -> Option<u64> {
    raw.trim()
        .parse::<u64>()
        .ok()
        .filter(|s| *s <= MAX_CLOCK_SECS)
}

/// The clock for a number of seconds; 0 is no clock at all.
fn clock_of(seconds: u64) -> Option<Duration> {
    (seconds > 0).then(|| Duration::from_secs(seconds))
}

/// The lowercase `scheme://host[:port]` form of an origin, or `None` if `raw` is not one.
fn normalize_origin(raw: &str) -> Option<String> {
    let lower = raw.trim().trim_end_matches('/').to_ascii_lowercase();
    let rest = lower
        .strip_prefix("https://")
        .or_else(|| lower.strip_prefix("http://"))?;
    let (host, port) = match rest.rsplit_once(':') {
        Some((host, port)) if !port.contains(']') => (host, Some(port)),
        _ => (rest, None),
    };
    let host_ok = !host.is_empty()
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '[' | ']'));
    let port_ok =
        port.is_none_or(|p| (1..=5).contains(&p.len()) && p.bytes().all(|b| b.is_ascii_digit()));
    (host_ok && port_ok).then_some(lower)
}

#[cfg(test)]
mod tests {
    //! Goal: the environment table maps to the right settings, blank values mean defaults, and every malformed value is refused with its own error.
    use super::*;
    use rstest::rstest;

    fn load(pairs: &[(&str, &str)]) -> Result<ServerConfig, ConfigError> {
        ServerConfig::from_lookup(|key| {
            pairs
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| (*v).to_owned())
        })
    }

    #[test]
    fn defaults_are_local_only_and_unseeded() {
        let config = load(&[]).unwrap();
        assert_eq!(config, ServerConfig::default());
        assert_eq!(config.bind.to_string(), "127.0.0.1:8080");
        assert_eq!(config.allowed_origins, Vec::<String>::new());
        assert_eq!(config.max_games, 100);
        assert_eq!(config.fixed_seed, None);
        assert_eq!(config.socket.join_timeout, Duration::from_secs(10));
        assert_eq!(config.socket.ping_every, Duration::from_secs(20));
        assert_eq!(config.socket.idle_timeout, Duration::from_secs(60));
        assert_eq!(config.socket.max_frame, 64 * 1024);
    }

    #[rstest]
    #[case("0.0.0.0:9000", "0.0.0.0:9000")]
    #[case(" 127.0.0.1:1 ", "127.0.0.1:1")]
    #[case("[::1]:8081", "[::1]:8081")]
    #[case("", "127.0.0.1:8080")]
    #[case("   ", "127.0.0.1:8080")]
    fn bind_addresses(#[case] text: &str, #[case] expected: &str) {
        let config = load(&[("MTG_BIND", text)]).unwrap();
        assert_eq!(config.bind.to_string(), expected);
    }

    #[rstest]
    #[case("localhost:8080")]
    #[case("8080")]
    #[case("1.2.3.4")]
    #[case("1.2.3.4:99999")]
    #[case("http://127.0.0.1:8080")]
    fn bad_bind_addresses(#[case] text: &str) {
        assert_eq!(
            load(&[("MTG_BIND", text)]),
            Err(ConfigError::BadBind(text.to_owned()))
        );
    }

    #[rstest]
    #[case("1", 1)]
    #[case("250", 250)]
    #[case(" 7 ", 7)]
    #[case("", 100)]
    fn game_caps(#[case] text: &str, #[case] expected: usize) {
        assert_eq!(
            load(&[("MTG_MAX_GAMES", text)]).unwrap().max_games,
            expected
        );
    }

    #[rstest]
    #[case("0")]
    #[case("-1")]
    #[case("ten")]
    #[case("1.5")]
    #[case("99999999999999999999999")]
    fn bad_game_caps(#[case] text: &str) {
        assert_eq!(
            load(&[("MTG_MAX_GAMES", text)]),
            Err(ConfigError::BadMaxGames(text.to_owned()))
        );
    }

    #[rstest]
    #[case("", &[])]
    #[case(" , ,, ", &[])]
    #[case("https://magic.example", &["https://magic.example"])]
    #[case("HTTPS://Magic.Example/", &["https://magic.example"])]
    #[case("https://a.example, http://localhost:5173", &["https://a.example", "http://localhost:5173"])]
    #[case("http://[::1]:5173,http://[::1]:5173", &["http://[::1]:5173"])]
    fn origin_lists(#[case] text: &str, #[case] expected: &[&str]) {
        let config = load(&[("MTG_ALLOWED_ORIGINS", text)]).unwrap();
        assert_eq!(config.allowed_origins, expected);
    }

    #[rstest]
    #[case("*")]
    #[case("magic.example")]
    #[case("ftp://magic.example")]
    #[case("https://magic.example/play")]
    #[case("https://user@magic.example")]
    #[case("https://magic.example:port")]
    #[case("https://magic.example:123456")]
    #[case("https://")]
    #[case("null")]
    fn bad_origins(#[case] entry: &str) {
        let text = format!("https://fine.example,{entry}");
        assert_eq!(
            load(&[("MTG_ALLOWED_ORIGINS", &text)]),
            Err(ConfigError::BadOrigin(entry.to_owned()))
        );
    }

    #[test]
    fn variables_combine() {
        let config = load(&[
            ("MTG_BIND", "0.0.0.0:80"),
            ("MTG_MAX_GAMES", "5"),
            ("MTG_ALLOWED_ORIGINS", "https://x.example"),
        ])
        .unwrap();
        assert_eq!(config.bind.port(), 80);
        assert_eq!(config.max_games, 5);
        assert_eq!(config.allowed_origins, ["https://x.example"]);
    }

    #[test]
    fn the_pace_defaults_skip_idle_seats_and_run_the_clocks() {
        let pace = load(&[]).unwrap().game.pace;
        assert!(pace.auto_pass);
        assert_eq!(pace.response, Some(Duration::from_secs(10)));
        assert_eq!(pace.blocks, Some(Duration::from_secs(45)));
    }

    #[rstest]
    #[case("1", true)]
    #[case("0", false)]
    #[case(" 0 ", false)]
    #[case("off", false)]
    #[case("OFF", false)]
    #[case("false", false)]
    #[case("on", true)]
    #[case("True", true)]
    #[case("", true)]
    #[case("  ", true)]
    fn auto_pass_switch(#[case] text: &str, #[case] expected: bool) {
        let config = load(&[("MTG_AUTO_PASS", text)]).unwrap();
        assert_eq!(config.game.pace.auto_pass, expected);
    }

    #[rstest]
    #[case("2")]
    #[case("yes")]
    #[case("-1")]
    #[case("0.5")]
    #[case("disabled")]
    fn bad_auto_pass_switch(#[case] text: &str) {
        assert_eq!(
            load(&[("MTG_AUTO_PASS", text)]),
            Err(ConfigError::BadAutoPass(text.to_owned()))
        );
    }

    #[rstest]
    #[case("1", Some(1))]
    #[case("30", Some(30))]
    #[case(" 7 ", Some(7))]
    #[case("86400", Some(86_400))]
    #[case("0", None)]
    #[case("", Some(10))]
    fn response_clock(#[case] text: &str, #[case] seconds: Option<u64>) {
        let pace = load(&[("MTG_RESPONSE_SECS", text)]).unwrap().game.pace;
        assert_eq!(pace.response, seconds.map(Duration::from_secs));
        assert_eq!(
            pace.blocks,
            Some(Duration::from_secs(45)),
            "the other clock is untouched"
        );
    }

    #[rstest]
    #[case("120", Some(120))]
    #[case("0", None)]
    #[case("", Some(45))]
    fn block_clock(#[case] text: &str, #[case] seconds: Option<u64>) {
        let pace = load(&[("MTG_BLOCK_SECS", text)]).unwrap().game.pace;
        assert_eq!(pace.blocks, seconds.map(Duration::from_secs));
        assert_eq!(
            pace.response,
            Some(Duration::from_secs(10)),
            "the other clock is untouched"
        );
    }

    #[rstest]
    #[case("-1")]
    #[case("ten")]
    #[case("1.5")]
    #[case("86401")]
    #[case("99999999999999999999999")]
    fn bad_clocks(#[case] text: &str) {
        assert_eq!(
            load(&[("MTG_RESPONSE_SECS", text)]),
            Err(ConfigError::BadResponseSecs(text.to_owned()))
        );
        assert_eq!(
            load(&[("MTG_BLOCK_SECS", text)]),
            Err(ConfigError::BadBlockSecs(text.to_owned()))
        );
    }
}
