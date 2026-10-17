//! Splits newline-delimited JSON (Scryfall's current bulk format) into per-line bytes.

use std::io::{BufRead, BufReader, Read};

use super::array_stream::{DEFAULT_MAX_ELEMENT_BYTES, StreamError};

/// Yields each non-blank line of a JSONL file as raw bytes.
pub struct LineStream<R: Read> {
    reader: BufReader<R>,
    max_line_bytes: usize,
    done: bool,
}

impl<R: Read> LineStream<R> {
    /// Wraps `reader`, which should yield one JSON object per line.
    pub fn new(reader: R) -> Self {
        Self {
            reader: BufReader::with_capacity(64 * 1024, reader),
            max_line_bytes: DEFAULT_MAX_ELEMENT_BYTES,
            done: false,
        }
    }

    /// Overrides the per-line size cap.
    #[must_use]
    pub fn with_max_line_bytes(mut self, limit: usize) -> Self {
        self.max_line_bytes = limit;
        self
    }

    /// Returns the next non-blank line without its terminator, or `None` at end of input or after any error.
    pub fn next_line(&mut self) -> Result<Option<Vec<u8>>, StreamError> {
        let result = self.read_line();
        if result.is_err() {
            self.done = true;
        }
        result
    }

    fn read_line(&mut self) -> Result<Option<Vec<u8>>, StreamError> {
        while !self.done {
            let mut line = Vec::new();
            let mut limited = (&mut self.reader).take(self.max_line_bytes as u64 + 1);
            let read = limited.read_until(b'\n', &mut line)?;
            if read == 0 {
                self.done = true;
                return Ok(None);
            }
            if line.len() > self.max_line_bytes && line.last() != Some(&b'\n') {
                return Err(StreamError::ElementTooLarge {
                    limit: self.max_line_bytes,
                });
            }
            if line.starts_with(&[0xEF, 0xBB, 0xBF]) {
                line.drain(..3);
            }
            let trimmed = line.trim_ascii();
            if !trimmed.is_empty() {
                return Ok(Some(trimmed.to_vec()));
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    //! Goal: JSONL framing yields exactly the non-blank lines for any line ending, BOM or chunking, and rejects oversized lines.
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    fn collect(input: &[u8]) -> Result<Vec<Vec<u8>>, StreamError> {
        let mut stream = LineStream::new(input);
        let mut lines = Vec::new();
        while let Some(line) = stream.next_line()? {
            lines.push(line);
        }
        Ok(lines)
    }

    #[rstest]
    #[case::empty("", &[])]
    #[case::single_no_newline("{\"a\":1}", &["{\"a\":1}"])]
    #[case::lf("{\"a\":1}\n{\"b\":2}\n", &["{\"a\":1}", "{\"b\":2}"])]
    #[case::crlf("{\"a\":1}\r\n{\"b\":2}\r\n", &["{\"a\":1}", "{\"b\":2}"])]
    #[case::blank_lines("\n\n{\"a\":1}\n   \n{\"b\":2}\n\n", &["{\"a\":1}", "{\"b\":2}"])]
    #[case::bom("\u{feff}{\"a\":1}\n", &["{\"a\":1}"])]
    fn splits_lines(#[case] input: &str, #[case] expected: &[&str]) {
        let lines = collect(input.as_bytes()).unwrap();
        assert_eq!(
            lines,
            expected
                .iter()
                .map(|l| l.as_bytes().to_vec())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn oversized_line_is_fatal_and_stops_the_stream() {
        let mut stream = LineStream::new(&b"{\"a\":1}\n0123456789012345\n{\"b\":2}\n"[..])
            .with_max_line_bytes(10);
        assert_eq!(stream.next_line().unwrap(), Some(b"{\"a\":1}".to_vec()));
        assert!(matches!(
            stream.next_line(),
            Err(StreamError::ElementTooLarge { limit: 10 })
        ));
        assert_eq!(stream.next_line().unwrap(), None);
    }

    #[test]
    fn line_exactly_at_the_cap_is_accepted() {
        let mut stream = LineStream::new(&b"0123456789\n"[..]).with_max_line_bytes(10);
        assert_eq!(stream.next_line().unwrap(), Some(b"0123456789".to_vec()));
    }

    proptest! {
        #[test]
        fn roundtrips_any_line_set_regardless_of_terminator(
            lines in proptest::collection::vec("[a-z{}\":0-9 ]{1,40}", 0..30),
            crlf in any::<bool>(),
        ) {
            let sep = if crlf { "\r\n" } else { "\n" };
            let input: String = lines.iter().flat_map(|l| [l.as_str(), sep]).collect();
            let expected: Vec<Vec<u8>> = lines
                .iter()
                .map(|l| l.trim().as_bytes().to_vec())
                .filter(|l| !l.is_empty())
                .collect();
            prop_assert_eq!(collect(input.as_bytes()).unwrap(), expected);
        }
    }
}
