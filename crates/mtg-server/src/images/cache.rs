//! The disk cache: one file per picture, written atomically, the least recently used dropped first.

use std::collections::{BTreeMap, HashMap};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::SystemTime;

use tokio::sync::OnceCell;

/// What a file the cache wrote starts its name with while it is being written.
const TEMP_PREFIX: &str = ".tmp-";

/// What the cache knows of one file.
#[derive(Debug, Clone, Copy)]
struct Entry {
    size: u64,
    tick: u64,
}

/// The files of the cache in the order of their last use, and their total size.
#[derive(Debug, Default)]
struct Index {
    files: HashMap<String, Entry>,
    /// Key by tick: the smallest tick is the least recently used file.
    order: BTreeMap<u64, String>,
    total: u64,
    tick: u64,
}

impl Index {
    fn next_tick(&mut self) -> u64 {
        self.tick += 1;
        self.tick
    }

    /// Records a file as the most recently used one, replacing an earlier entry of the key.
    fn insert(&mut self, key: &str, size: u64) {
        self.remove(key);
        let tick = self.next_tick();
        self.files.insert(key.to_owned(), Entry { size, tick });
        self.order.insert(tick, key.to_owned());
        self.total += size;
    }

    /// Marks a file as just used; false when the index does not hold it.
    fn touch(&mut self, key: &str) -> bool {
        let Some(size) = self.files.get(key).map(|entry| entry.size) else {
            return false;
        };
        self.insert(key, size);
        true
    }

    fn remove(&mut self, key: &str) {
        if let Some(entry) = self.files.remove(key) {
            self.order.remove(&entry.tick);
            self.total -= entry.size;
        }
    }

    /// Drops the least recently used files until the total is at most `limit`; returns their keys.
    fn evict_to(&mut self, limit: u64) -> Vec<String> {
        let mut dropped = Vec::new();
        while self.total > limit {
            let Some((_, key)) = self.order.pop_first() else {
                break;
            };
            if let Some(entry) = self.files.remove(&key) {
                self.total -= entry.size;
            }
            dropped.push(key);
        }
        dropped
    }
}

/// A directory of pictures that stays under a size limit.
#[derive(Debug)]
pub(crate) struct DiskCache {
    dir: PathBuf,
    max_bytes: u64,
    index: Mutex<Index>,
    loaded: OnceCell<()>,
    temps: AtomicU64,
}

impl DiskCache {
    /// A cache in `dir` that holds at most `max_bytes`; the directory is read when first used.
    pub(crate) fn new(dir: PathBuf, max_bytes: u64) -> DiskCache {
        DiskCache {
            dir,
            max_bytes,
            index: Mutex::new(Index::default()),
            loaded: OnceCell::new(),
            temps: AtomicU64::new(0),
        }
    }

    fn index(&self) -> MutexGuard<'_, Index> {
        // Nothing here can leave the index half-updated, so a poisoned lock is still sound.
        self.index.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The bytes of `key`, and the file counts as just used; `None` when the cache lacks it.
    pub(crate) async fn get(&self, key: &str) -> Option<Vec<u8>> {
        self.load().await;
        if !self.index().touch(key) {
            return None;
        }
        match tokio::fs::read(self.dir.join(key)).await {
            Ok(bytes) => Some(bytes),
            Err(error) => {
                if error.kind() != io::ErrorKind::NotFound {
                    tracing::warn!(%error, key, "could not read a cached picture");
                }
                self.index().remove(key);
                None
            }
        }
    }

    /// Whether the cache holds `key`, without counting it as used.
    pub(crate) async fn contains(&self, key: &str) -> bool {
        self.load().await;
        self.index().files.contains_key(key)
    }

    /// Stores `bytes` as `key`: written to a temporary file and renamed, so a reader or a crash
    /// never sees half a picture. Then the least recently used files go until the limit holds.
    pub(crate) async fn put(&self, key: &str, bytes: &[u8]) -> io::Result<()> {
        self.load().await;
        tokio::fs::create_dir_all(&self.dir).await?;
        let temp = self.dir.join(format!(
            "{TEMP_PREFIX}{}-{}",
            std::process::id(),
            self.temps.fetch_add(1, Ordering::Relaxed)
        ));
        let written = async {
            tokio::fs::write(&temp, bytes).await?;
            tokio::fs::rename(&temp, self.dir.join(key)).await
        }
        .await;
        if let Err(error) = written {
            let _ = tokio::fs::remove_file(&temp).await;
            return Err(error);
        }
        let dropped = {
            let mut index = self.index();
            index.insert(key, bytes.len() as u64);
            index.evict_to(self.max_bytes)
        };
        self.delete(&dropped).await;
        Ok(())
    }

    async fn delete(&self, keys: &[String]) {
        for key in keys {
            if let Err(error) = tokio::fs::remove_file(self.dir.join(key)).await
                && error.kind() != io::ErrorKind::NotFound
            {
                tracing::warn!(%error, key, "could not delete a cached picture");
            }
        }
    }

    /// Reads the directory once: older files are used less recently, leftovers of an interrupted
    /// write are deleted and a limit lowered since the last run is applied.
    async fn load(&self) {
        self.loaded
            .get_or_init(|| async {
                let found = scan(&self.dir).await;
                let dropped = {
                    let mut index = self.index();
                    for (_, key, size) in found {
                        index.insert(&key, size);
                    }
                    index.evict_to(self.max_bytes)
                };
                self.delete(&dropped).await;
            })
            .await;
    }
}

/// The pictures in `dir` as `(modified, name, size)`, oldest first; temporary files are deleted.
async fn scan(dir: &Path) -> Vec<(SystemTime, String, u64)> {
    let mut found = Vec::new();
    let Ok(mut entries) = tokio::fs::read_dir(dir).await else {
        return found;
    };
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(meta) = entry.metadata().await else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        if name.starts_with(TEMP_PREFIX) {
            let _ = tokio::fs::remove_file(entry.path()).await;
        } else if !name.starts_with('.') {
            found.push((
                meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                name,
                meta.len(),
            ));
        }
    }
    found.sort();
    found
}

#[cfg(test)]
mod tests {
    //! Goal: the index drops the least recently used files first and keeps its total exact; the cache stores whole files only, survives a restart in the order of the files' ages and enforces the limit however pictures arrive.
    use std::time::Duration;

    use rstest::rstest;

    use super::*;

    fn keys(index: &Index) -> Vec<&str> {
        index.order.values().map(String::as_str).collect()
    }

    #[rstest]
    #[case::fits(&[("a", 10), ("b", 10)], 20, &[], &["a", "b"])]
    #[case::oldest_first(&[("a", 10), ("b", 10), ("c", 10)], 20, &["a"], &["b", "c"])]
    #[case::several(&[("a", 10), ("b", 10), ("c", 10)], 10, &["a", "b"], &["c"])]
    #[case::too_big_for_the_limit(&[("a", 10), ("b", 50)], 40, &["a", "b"], &[])]
    #[case::rewrite_makes_it_newest(&[("a", 10), ("b", 10), ("a", 10)], 10, &["b"], &["a"])]
    #[case::rewrite_changes_the_size(&[("a", 10), ("b", 10), ("a", 25)], 30, &["b"], &["a"])]
    #[case::nothing_allowed(&[("a", 1)], 0, &["a"], &[])]
    fn eviction_goes_in_order_of_use(
        #[case] puts: &[(&str, u64)],
        #[case] limit: u64,
        #[case] dropped: &[&str],
        #[case] kept: &[&str],
    ) {
        let mut index = Index::default();
        for (key, size) in puts {
            index.insert(key, *size);
        }
        assert_eq!(index.evict_to(limit), dropped);
        assert_eq!(keys(&index), kept);
        let held: u64 = index.files.values().map(|entry| entry.size).sum();
        assert_eq!(index.total, held);
        assert!(index.total <= limit);
    }

    #[test]
    fn a_read_makes_a_file_the_newest() {
        let mut index = Index::default();
        for key in ["a", "b", "c"] {
            index.insert(key, 10);
        }
        assert!(index.touch("a"));
        assert!(!index.touch("zzz"));
        assert_eq!(index.evict_to(20), ["b"]);
        assert_eq!(keys(&index), ["c", "a"]);
    }

    fn cache(dir: &tempfile::TempDir, max: u64) -> DiskCache {
        DiskCache::new(dir.path().join("pictures"), max)
    }

    #[tokio::test]
    async fn a_picture_is_stored_and_read_back() {
        let dir = tempfile::tempdir().unwrap();
        let cache = cache(&dir, 1000);
        assert_eq!(cache.get("a.jpg").await, None);
        cache.put("a.jpg", b"one").await.unwrap();
        assert_eq!(cache.get("a.jpg").await.as_deref(), Some(&b"one"[..]));
        assert!(cache.contains("a.jpg").await);
        assert!(!cache.contains("b.jpg").await);
    }

    #[tokio::test]
    async fn the_limit_holds_after_every_put_and_the_oldest_files_go() {
        let dir = tempfile::tempdir().unwrap();
        let cache = cache(&dir, 25);
        for key in ["a", "b", "c", "d"] {
            cache.put(key, &[0; 10]).await.unwrap();
        }
        assert!(cache.get("a").await.is_none() && cache.get("b").await.is_none());
        assert!(cache.get("c").await.is_some() && cache.get("d").await.is_some());
        let on_disk = std::fs::read_dir(dir.path().join("pictures"))
            .unwrap()
            .count();
        assert_eq!(on_disk, 2, "the evicted files are deleted");
    }

    #[tokio::test]
    async fn reading_a_file_saves_it_from_eviction() {
        let dir = tempfile::tempdir().unwrap();
        let cache = cache(&dir, 25);
        cache.put("a", &[0; 10]).await.unwrap();
        cache.put("b", &[0; 10]).await.unwrap();
        assert!(cache.get("a").await.is_some());
        cache.put("c", &[0; 10]).await.unwrap();
        assert!(cache.get("b").await.is_none());
        assert!(cache.get("a").await.is_some() && cache.get("c").await.is_some());
    }

    #[tokio::test]
    async fn a_failed_write_leaves_no_file_and_no_entry() {
        let dir = tempfile::tempdir().unwrap();
        let cache = cache(&dir, 1000);
        // A directory in the way of the final name makes the rename fail after the write.
        std::fs::create_dir_all(dir.path().join("pictures").join("a.jpg")).unwrap();
        assert!(cache.put("a.jpg", b"one").await.is_err());
        assert!(!cache.contains("a.jpg").await);
        let leftovers: Vec<String> = std::fs::read_dir(dir.path().join("pictures"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(leftovers, ["a.jpg"], "no temporary file stays");
    }

    #[tokio::test]
    async fn a_restart_reads_the_directory_oldest_first_and_cleans_up() {
        let dir = tempfile::tempdir().unwrap();
        let pictures = dir.path().join("pictures");
        std::fs::create_dir_all(&pictures).unwrap();
        let now = SystemTime::now();
        for (name, age) in [("old", 300), ("mid", 200), ("new", 100)] {
            let file = std::fs::File::create(pictures.join(name)).unwrap();
            std::io::Write::write_all(&mut &file, &[0; 10]).unwrap();
            file.set_modified(now - Duration::from_secs(age)).unwrap();
        }
        std::fs::write(pictures.join(".tmp-1-1"), b"half").unwrap();
        let cache = DiskCache::new(pictures.clone(), 25);
        assert!(cache.get("new").await.is_some() && cache.get("mid").await.is_some());
        assert!(
            cache.get("old").await.is_none(),
            "the oldest went to meet the limit"
        );
        assert!(!pictures.join(".tmp-1-1").exists());
        assert!(!pictures.join("old").exists());
    }

    #[tokio::test]
    async fn a_file_removed_behind_the_cache_is_a_miss() {
        let dir = tempfile::tempdir().unwrap();
        let cache = cache(&dir, 1000);
        cache.put("a", b"x").await.unwrap();
        std::fs::remove_file(dir.path().join("pictures").join("a")).unwrap();
        assert!(cache.get("a").await.is_none());
        assert!(!cache.contains("a").await);
    }
}
