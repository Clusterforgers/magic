//! Settings of the picture cache and of its upstream requests.

use std::path::PathBuf;
use std::time::Duration;

use crate::config::ConfigError;

/// Size limit of the cache, in MiB, when `MTG_IMAGE_CACHE_MIB` is unset.
pub const DEFAULT_CACHE_MIB: u64 = 2048;
/// Largest `MTG_IMAGE_CACHE_MIB` accepted: 1 TiB.
pub const MAX_CACHE_MIB: u64 = 1024 * 1024;
/// The smallest gap between two upstream requests; Scryfall asks for 50 to 100 ms.
pub const MIN_GAP: Duration = Duration::from_millis(100);

/// Where pictures are kept, how many, and how the upstream is asked for them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageConfig {
    /// The directory of the cache (`MTG_IMAGE_CACHE`).
    pub cache_dir: PathBuf,
    /// Bytes the cache may hold before it drops the least recently used pictures (`MTG_IMAGE_CACHE_MIB`).
    pub max_bytes: u64,
    /// `scheme://host[:port]` that upstream requests go to instead of the host a stored link names;
    /// the stored link is validated all the same. For tests only: never read from the environment.
    pub upstream_base: Option<String>,
    /// Time left between the end of one upstream request and the start of the next.
    pub min_gap: Duration,
    /// Tries per picture, the first included.
    pub attempts: u32,
    /// Wait before the second try; it doubles for each further one.
    pub backoff: Duration,
    /// Longest one upstream request may take.
    pub timeout: Duration,
}

impl Default for ImageConfig {
    fn default() -> Self {
        ImageConfig {
            cache_dir: std::env::temp_dir().join("mtg-server").join("images"),
            max_bytes: DEFAULT_CACHE_MIB * 1024 * 1024,
            upstream_base: None,
            min_gap: MIN_GAP,
            attempts: 3,
            backoff: Duration::from_secs(2),
            timeout: Duration::from_secs(30),
        }
    }
}

impl ImageConfig {
    /// Reads `MTG_IMAGE_CACHE` and `MTG_IMAGE_CACHE_MIB` from `get`, which returns `None` for
    /// blank values. Without `MTG_IMAGE_CACHE` the cache lives under the user's cache location.
    pub(crate) fn from_lookup(
        get: &impl Fn(&str) -> Option<String>,
    ) -> Result<ImageConfig, ConfigError> {
        let mut config = ImageConfig::default();
        if let Some(text) = get("MTG_IMAGE_CACHE") {
            config.cache_dir = PathBuf::from(text.trim());
        } else if let Some(dir) = default_dir(get) {
            config.cache_dir = dir;
        }
        if let Some(text) = get("MTG_IMAGE_CACHE_MIB") {
            let mib = match text.trim().parse::<u64>() {
                Ok(n) if (1..=MAX_CACHE_MIB).contains(&n) => n,
                _ => return Err(ConfigError::BadImageCacheMib(text)),
            };
            config.max_bytes = mib * 1024 * 1024;
        }
        Ok(config)
    }
}

/// `$XDG_CACHE_HOME/mtg-server/images`, else `$HOME/.cache/mtg-server/images`.
fn default_dir(get: &impl Fn(&str) -> Option<String>) -> Option<PathBuf> {
    let base = get("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .or_else(|| get("HOME").map(|home| PathBuf::from(home).join(".cache")))?;
    Some(base.join("mtg-server").join("images"))
}

#[cfg(test)]
mod tests {
    //! Goal: the environment maps to the cache directory and limit, the user's cache location is the default, and a bad size is refused.
    use rstest::rstest;

    use super::*;

    fn load(pairs: &[(&str, &str)]) -> Result<ImageConfig, ConfigError> {
        ImageConfig::from_lookup(&|key: &str| {
            pairs
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| (*v).to_owned())
        })
    }

    #[rstest]
    #[case::named(&[("MTG_IMAGE_CACHE", "/srv/img"), ("HOME", "/home/u")], "/srv/img")]
    #[case::xdg(&[("XDG_CACHE_HOME", "/x/cache"), ("HOME", "/home/u")], "/x/cache/mtg-server/images")]
    #[case::home(&[("HOME", "/home/u")], "/home/u/.cache/mtg-server/images")]
    #[case::relative_xdg_is_ignored(&[("XDG_CACHE_HOME", "cache"), ("HOME", "/home/u")], "/home/u/.cache/mtg-server/images")]
    fn the_directory_follows_the_environment(#[case] vars: &[(&str, &str)], #[case] dir: &str) {
        assert_eq!(load(vars).unwrap().cache_dir, PathBuf::from(dir));
    }

    #[test]
    fn nowhere_to_put_it_keeps_the_temporary_default() {
        assert_eq!(
            load(&[]).unwrap().cache_dir,
            ImageConfig::default().cache_dir
        );
    }

    #[rstest]
    #[case::default(None, Some(2048 * 1024 * 1024))]
    #[case::one(Some("1"), Some(1024 * 1024))]
    #[case::ten(Some(" 10 "), Some(10 * 1024 * 1024))]
    #[case::most(Some("1048576"), Some(1024 * 1024 * 1024 * 1024))]
    #[case::zero(Some("0"), None)]
    #[case::too_big(Some("1048577"), None)]
    #[case::words(Some("big"), None)]
    #[case::negative(Some("-4"), None)]
    fn the_limit_is_a_whole_number_of_mebibytes(
        #[case] text: Option<&str>,
        #[case] bytes: Option<u64>,
    ) {
        let pairs: Vec<(&str, &str)> = text
            .map(|t| ("MTG_IMAGE_CACHE_MIB", t))
            .into_iter()
            .collect();
        match (load(&pairs), bytes) {
            (Ok(config), Some(bytes)) => assert_eq!(config.max_bytes, bytes),
            (Err(ConfigError::BadImageCacheMib(_)), None) => {}
            (other, expected) => panic!("{other:?} against {expected:?}"),
        }
    }
}
