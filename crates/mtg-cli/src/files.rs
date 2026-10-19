//! Small file helpers: checksums, integrity checks, atomic writes and sizes for people.

use std::fs::{self, File};
use std::io::{self, BufReader, Read, Write};
use std::path::{Path, PathBuf};

use flate2::read::MultiGzDecoder;
use sha2::{Digest, Sha256};

/// Hexadecimal form of a SHA-256 digest.
pub fn hex(digest: &[u8]) -> String {
    use std::fmt::Write as _;
    digest
        .iter()
        .fold(String::with_capacity(64), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        })
}

/// SHA-256 of some bytes, in hexadecimal.
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

/// SHA-256 of a file in hexadecimal and its length; reads in blocks, so the file may be large.
pub fn sha256_file(path: &Path) -> io::Result<(String, u64)> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut block = vec![0u8; 256 * 1024];
    let mut length = 0u64;
    loop {
        let read = file.read(&mut block)?;
        if read == 0 {
            return Ok((hex(&hasher.finalize()), length));
        }
        hasher.update(&block[..read]);
        length += u64::try_from(read).unwrap_or(0);
    }
}

/// Whether the file starts with the gzip magic number.
pub fn is_gzip(path: &Path) -> io::Result<bool> {
    let mut magic = [0u8; 2];
    let read = File::open(path)?.read(&mut magic)?;
    Ok(read == 2 && magic == [0x1f, 0x8b])
}

/// Decompresses a gzip file to nowhere, which checks its length and its CRC.
pub fn verify_gzip(path: &Path) -> io::Result<()> {
    let mut decoder = MultiGzDecoder::new(BufReader::new(File::open(path)?));
    io::copy(&mut decoder, &mut io::sink()).map(|_| ())
}

/// Writes `bytes` to `path` through a temporary file in the same directory and a rename.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut name = path
        .file_name()
        .map(std::ffi::OsStr::to_os_string)
        .unwrap_or_default();
    name.push(format!(".tmp-{}", std::process::id()));
    let temporary: PathBuf = path.with_file_name(name);
    let written = File::create(&temporary).and_then(|mut file| {
        file.write_all(bytes)?;
        file.sync_all()
    });
    match written.and_then(|()| fs::rename(&temporary, path)) {
        Ok(()) => Ok(()),
        Err(error) => {
            let _ = fs::remove_file(&temporary);
            Err(error)
        }
    }
}

/// A size such as `23.9 MiB`.
#[allow(clippy::cast_precision_loss)] // sizes shown to a person need no more digits
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// The first twelve characters of a checksum, for lines a person reads.
pub fn short(checksum: &str) -> &str {
    checksum.get(..12).unwrap_or(checksum)
}

#[cfg(test)]
mod tests {
    //! Goal: checksums, gzip checks and atomic writes behave on files of any size, and sizes print the way a person reads them.
    use std::io::Write;

    use flate2::Compression;
    use flate2::write::GzEncoder;
    use proptest::prelude::*;
    use rstest::rstest;

    use super::*;

    fn gzip(data: &[u8]) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data).unwrap();
        encoder.finish().unwrap()
    }

    #[rstest]
    #[case(
        b"",
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    )]
    #[case(
        b"abc",
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    )]
    fn sha256_matches_the_known_digests(#[case] data: &[u8], #[case] expected: &str) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f");
        fs::write(&path, data).unwrap();
        assert_eq!(sha256_hex(data), expected);
        assert_eq!(
            sha256_file(&path).unwrap(),
            (expected.to_owned(), data.len() as u64)
        );
    }

    #[test]
    fn a_file_longer_than_one_block_hashes_like_its_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("big");
        let data: Vec<u8> = (0..700_000u32).map(|n| (n % 251) as u8).collect();
        fs::write(&path, &data).unwrap();
        assert_eq!(sha256_file(&path).unwrap(), (sha256_hex(&data), 700_000));
    }

    #[rstest]
    #[case::whole(false, None, true)]
    #[case::cut_short(true, Some(10), false)]
    #[case::flipped_byte(false, Some(0), false)]
    fn gzip_integrity_is_checked_to_the_end(
        #[case] cut: bool,
        #[case] at: Option<usize>,
        #[case] intact: bool,
    ) {
        let data: Vec<u8> = (0..50_000u32)
            .map(|n| u8::try_from(n * 7 % 256).unwrap())
            .collect();
        let mut packed = gzip(&data);
        if let (true, Some(keep)) = (cut, at) {
            packed.truncate(packed.len() - keep);
        } else if let Some(position) = at {
            let middle = packed.len() / 2 + position;
            packed[middle] ^= 0xff;
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f.gz");
        fs::write(&path, &packed).unwrap();
        assert!(is_gzip(&path).unwrap());
        assert_eq!(verify_gzip(&path).is_ok(), intact);
    }

    #[test]
    fn plain_files_are_not_gzip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("f");
        for content in [&b""[..], b"[", b"[{\"a\":1}]"] {
            fs::write(&path, content).unwrap();
            assert!(!is_gzip(&path).unwrap());
        }
    }

    #[test]
    fn an_atomic_write_replaces_the_file_and_leaves_no_temporary() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("manifest.json");
        write_atomic(&path, b"one").unwrap();
        write_atomic(&path, b"two").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"two");
        let names: Vec<_> = fs::read_dir(dir.path()).unwrap().collect();
        assert_eq!(names.len(), 1);
    }

    #[test]
    fn an_atomic_write_into_a_missing_directory_fails_cleanly() {
        let dir = tempfile::tempdir().unwrap();
        assert!(write_atomic(&dir.path().join("no/such/dir/f"), b"x").is_err());
    }

    #[rstest]
    #[case(0, "0 B")]
    #[case(1023, "1023 B")]
    #[case(1024, "1.0 KiB")]
    #[case(25_012_034, "23.9 MiB")]
    #[case(5 * 1024 * 1024 * 1024, "5.0 GiB")]
    fn sizes_read_like_a_person_writes_them(#[case] bytes: u64, #[case] shown: &str) {
        assert_eq!(human_size(bytes), shown);
    }

    proptest! {
        #[test]
        fn hex_is_twice_as_long_as_the_bytes_and_lowercase(bytes in prop::collection::vec(any::<u8>(), 0..64)) {
            let text = hex(&bytes);
            prop_assert_eq!(text.len(), bytes.len() * 2);
            prop_assert!(text.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)));
        }
    }
}
