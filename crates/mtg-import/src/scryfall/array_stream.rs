//! Splits a JSON array of objects into per-element bytes without buffering the whole input.

use std::io::{self, BufRead, BufReader, Read};

use thiserror::Error;

/// Default cap on the byte size of one array element.
pub const DEFAULT_MAX_ELEMENT_BYTES: usize = 8 * 1024 * 1024;

/// Matches `serde_json`'s own recursion limit so deeply nested junk is rejected early.
const MAX_NESTING: usize = 128;

/// Structural failures after which the rest of the array cannot be read.
#[derive(Debug, Error)]
pub enum StreamError {
    /// The underlying reader failed.
    #[error("read failed: {0}")]
    Io(#[from] io::Error),
    /// The input does not start with `[`.
    #[error("expected a JSON array at byte {offset}")]
    NotAnArray {
        /// Byte offset of the first unexpected byte.
        offset: u64,
    },
    /// The input ended before the array was closed.
    #[error("unexpected end of input")]
    UnexpectedEof,
    /// A byte that cannot appear at this point of an array of objects.
    #[error("unexpected byte {byte:#04x} at offset {offset}")]
    Unexpected {
        /// The offending byte.
        byte: u8,
        /// Byte offset of the offending byte.
        offset: u64,
    },
    /// One element exceeded the configured size cap.
    #[error("array element larger than {limit} bytes")]
    ElementTooLarge {
        /// The configured cap in bytes.
        limit: usize,
    },
}

/// Reader wrapper that retries interrupted reads so buffered fills never surface them.
struct RetryRead<R>(R);

impl<R: Read> Read for RetryRead<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            match self.0.read(buf) {
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                other => return other,
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Start,
    BeforeFirst,
    AfterElement,
    AfterComma,
    Done,
}

/// A byte inside an object that breaks bracket matching or nests too deeply.
struct Fault {
    index: usize,
    byte: u8,
}

/// Tracks string/bracket state to find where one JSON object ends.
#[derive(Default)]
struct ObjectScanner {
    in_string: bool,
    escaped: bool,
    closers: Vec<u8>,
}

impl ObjectScanner {
    fn reset(&mut self) {
        self.in_string = false;
        self.escaped = false;
        self.closers.clear();
    }

    /// Consumes `chunk` until the object closes; returns the bytes used when it does.
    fn feed(&mut self, chunk: &[u8]) -> Result<Option<usize>, Fault> {
        for (index, &byte) in chunk.iter().enumerate() {
            if self.in_string {
                match byte {
                    _ if self.escaped => self.escaped = false,
                    b'\\' => self.escaped = true,
                    b'"' => self.in_string = false,
                    _ => {}
                }
                continue;
            }
            match byte {
                b'"' => self.in_string = true,
                b'{' | b'[' => {
                    if self.closers.len() >= MAX_NESTING {
                        return Err(Fault { index, byte });
                    }
                    self.closers.push(if byte == b'{' { b'}' } else { b']' });
                }
                b'}' | b']' => {
                    if self.closers.pop() != Some(byte) {
                        return Err(Fault { index, byte });
                    }
                    if self.closers.is_empty() {
                        return Ok(Some(index + 1));
                    }
                }
                _ => {}
            }
        }
        Ok(None)
    }
}

/// Yields each top-level object of a JSON array as raw bytes.
pub struct ArrayStream<R: Read> {
    reader: BufReader<RetryRead<R>>,
    state: State,
    offset: u64,
    max_element_bytes: usize,
    scanner: ObjectScanner,
}

impl<R: Read> ArrayStream<R> {
    /// Wraps `reader`, which should yield a JSON array of objects.
    pub fn new(reader: R) -> Self {
        Self {
            reader: BufReader::with_capacity(64 * 1024, RetryRead(reader)),
            state: State::Start,
            offset: 0,
            max_element_bytes: DEFAULT_MAX_ELEMENT_BYTES,
            scanner: ObjectScanner::default(),
        }
    }

    /// Overrides the per-element size cap.
    #[must_use]
    pub fn with_max_element_bytes(mut self, limit: usize) -> Self {
        self.max_element_bytes = limit;
        self
    }

    /// Returns the next element's bytes, or `None` after the closing `]` or any earlier error.
    pub fn next_element(&mut self) -> Result<Option<Vec<u8>>, StreamError> {
        let result = self.advance();
        if result.is_err() {
            self.state = State::Done;
        }
        result
    }

    fn advance(&mut self) -> Result<Option<Vec<u8>>, StreamError> {
        loop {
            match self.state {
                State::Done => return Ok(None),
                State::Start => {
                    self.skip_bom()?;
                    match self.peek_significant()? {
                        Some(b'[') => {
                            self.consume(1);
                            self.state = State::BeforeFirst;
                        }
                        Some(_) => {
                            return Err(StreamError::NotAnArray {
                                offset: self.offset,
                            });
                        }
                        None => return Err(StreamError::UnexpectedEof),
                    }
                }
                State::BeforeFirst | State::AfterElement | State::AfterComma => {
                    let byte = self.peek_significant()?.ok_or(StreamError::UnexpectedEof)?;
                    match (self.state, byte) {
                        (State::BeforeFirst | State::AfterElement, b']') => {
                            self.consume(1);
                            self.state = State::Done;
                            return Ok(None);
                        }
                        (State::AfterElement, b',') => {
                            self.consume(1);
                            self.state = State::AfterComma;
                        }
                        (State::BeforeFirst | State::AfterComma, b'{') => {
                            let element = self.read_object()?;
                            self.state = State::AfterElement;
                            return Ok(Some(element));
                        }
                        _ => {
                            return Err(StreamError::Unexpected {
                                byte,
                                offset: self.offset,
                            });
                        }
                    }
                }
            }
        }
    }

    fn consume(&mut self, amount: usize) {
        self.reader.consume(amount);
        self.offset += amount as u64;
    }

    /// Skips ASCII whitespace and returns the next byte without consuming it.
    fn peek_significant(&mut self) -> io::Result<Option<u8>> {
        loop {
            let buffer = self.reader.fill_buf()?;
            if buffer.is_empty() {
                return Ok(None);
            }
            let skip = buffer
                .iter()
                .take_while(|b| b.is_ascii_whitespace())
                .count();
            if let Some(&byte) = buffer.get(skip) {
                self.consume(skip);
                return Ok(Some(byte));
            }
            let all = buffer.len();
            self.consume(all);
        }
    }

    fn skip_bom(&mut self) -> Result<(), StreamError> {
        if self.peek_significant()? != Some(0xEF) {
            return Ok(());
        }
        for expected in [0xEF, 0xBB, 0xBF] {
            let buffer = self.reader.fill_buf()?;
            if buffer.first() != Some(&expected) {
                return Err(StreamError::NotAnArray {
                    offset: self.offset,
                });
            }
            self.consume(1);
        }
        Ok(())
    }

    fn read_object(&mut self) -> Result<Vec<u8>, StreamError> {
        let mut element = Vec::new();
        self.scanner.reset();
        loop {
            let chunk = self.reader.fill_buf()?;
            if chunk.is_empty() {
                return Err(StreamError::UnexpectedEof);
            }
            let finished = self.scanner.feed(chunk);
            let used = match &finished {
                Ok(Some(used)) => *used,
                Ok(None) => chunk.len(),
                Err(fault) => fault.index,
            };
            if element.len() + used > self.max_element_bytes {
                return Err(StreamError::ElementTooLarge {
                    limit: self.max_element_bytes,
                });
            }
            element.extend_from_slice(&chunk[..used]);
            self.consume(used);
            match finished {
                Ok(Some(_)) => return Ok(element),
                Ok(None) => {}
                Err(fault) => {
                    return Err(StreamError::Unexpected {
                        byte: fault.byte,
                        offset: self.offset,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Goal: the splitter finds exact element boundaries for any chunking of valid input and fails loudly on malformed framing.
    use super::*;
    use proptest::prelude::*;
    use rstest::rstest;

    /// Reader that hands out at most `chunk` bytes per call, to exercise buffer boundaries.
    struct ChunkReader<'a> {
        data: &'a [u8],
        chunk: usize,
    }

    impl Read for ChunkReader<'_> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let n = self.chunk.min(buf.len()).min(self.data.len());
            buf[..n].copy_from_slice(&self.data[..n]);
            self.data = &self.data[n..];
            Ok(n)
        }
    }

    fn collect(input: &[u8], chunk: usize) -> (Vec<String>, Option<StreamError>) {
        let mut stream = ArrayStream::new(ChunkReader { data: input, chunk });
        let mut items = Vec::new();
        loop {
            match stream.next_element() {
                Ok(Some(bytes)) => items.push(String::from_utf8_lossy(&bytes).into_owned()),
                Ok(None) => return (items, None),
                Err(e) => return (items, Some(e)),
            }
        }
    }

    #[rstest]
    #[case::empty_array("[]", &[])]
    #[case::padded_empty("  \n[ \t ]\r\n", &[])]
    #[case::single("[{\"a\":1}]", &["{\"a\":1}"])]
    #[case::two_with_whitespace("[ {\"a\":1} ,\n {\"b\":2}\r\n]", &["{\"a\":1}", "{\"b\":2}"])]
    #[case::brace_in_string("[{\"a\":\"}{][\"}]", &["{\"a\":\"}{][\"}"])]
    #[case::escaped_quote("[{\"a\":\"\\\"}\"}]", &["{\"a\":\"\\\"}\"}"])]
    #[case::escaped_backslash_before_quote("[{\"a\":\"\\\\\"},{\"b\":1}]", &["{\"a\":\"\\\\\"}", "{\"b\":1}"])]
    #[case::nested("[{\"a\":{\"b\":[1,{\"c\":[]}]}}]", &["{\"a\":{\"b\":[1,{\"c\":[]}]}}"])]
    #[case::bom("\u{feff}[{\"a\":1}]", &["{\"a\":1}"])]
    #[case::unicode("[{\"name\":\"Æther Vial ✦\"}]", &["{\"name\":\"Æther Vial ✦\"}"])]
    #[case::trailing_garbage_after_close("[{\"a\":1}] trailing", &["{\"a\":1}"])]
    fn splits_valid_arrays(#[case] input: &str, #[case] expected: &[&str]) {
        for chunk in [1, 2, 3, 7, 64, 1 << 16] {
            let (items, error) = collect(input.as_bytes(), chunk);
            assert!(error.is_none(), "chunk {chunk}: {error:?}");
            assert_eq!(items, expected, "chunk {chunk}");
        }
    }

    #[rstest]
    #[case::empty_input("")]
    #[case::only_whitespace("   \n")]
    #[case::unterminated_array("[{\"a\":1}")]
    #[case::unterminated_array_after_comma("[{\"a\":1},")]
    #[case::unterminated_object("[{\"a\":1")]
    #[case::unterminated_string("[{\"a\":\"abc")]
    #[case::not_an_array("{\"a\":1}")]
    #[case::scalar_array("[1,2]")]
    #[case::array_of_arrays("[[1]]")]
    #[case::trailing_comma("[{\"a\":1},]")]
    #[case::leading_comma("[,{\"a\":1}]")]
    #[case::missing_comma("[{\"a\":1}{\"b\":2}]")]
    #[case::mismatched_brackets("[{\"a\":[1}]")]
    #[case::stray_close("[{\"a\":1]]")]
    #[case::broken_bom("\u{feff}\u{feff}[]")]
    #[case::plain_text("hello world")]
    fn rejects_malformed_framing(#[case] input: &str) {
        for chunk in [1, 5, 1 << 16] {
            let (_, error) = collect(input.as_bytes(), chunk);
            assert!(error.is_some(), "chunk {chunk} accepted {input:?}");
        }
    }

    #[test]
    fn errors_finish_the_stream() {
        let mut stream = ArrayStream::new("[1]".as_bytes());
        assert!(stream.next_element().is_err());
        assert!(matches!(stream.next_element(), Ok(None)));
    }

    #[test]
    fn oversized_elements_are_rejected_not_buffered() {
        let input = format!("[{{\"a\":\"{}\"}}]", "x".repeat(1000));
        let mut stream = ArrayStream::new(input.as_bytes()).with_max_element_bytes(100);
        assert!(matches!(
            stream.next_element(),
            Err(StreamError::ElementTooLarge { limit: 100 })
        ));
    }

    #[test]
    fn excessive_nesting_is_rejected() {
        let input = format!("[{}{}]", "{\"a\":".repeat(200), "}".repeat(200));
        let (_, error) = collect(input.as_bytes(), 16);
        assert!(error.is_some());
    }

    #[test]
    fn reports_io_errors() {
        struct Failing;
        impl Read for Failing {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::other("disk on fire"))
            }
        }
        let mut stream = ArrayStream::new(Failing);
        assert!(matches!(stream.next_element(), Err(StreamError::Io(_))));
    }

    #[test]
    fn interrupted_reads_are_retried() {
        struct Flaky(bool, &'static [u8]);
        impl Read for Flaky {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                if !self.0 {
                    self.0 = true;
                    return Err(io::ErrorKind::Interrupted.into());
                }
                let n = buf.len().min(self.1.len());
                buf[..n].copy_from_slice(&self.1[..n]);
                self.1 = &self.1[n..];
                Ok(n)
            }
        }
        let mut stream = ArrayStream::new(Flaky(false, b"[{}]"));
        assert_eq!(stream.next_element().unwrap().unwrap(), b"{}");
    }

    fn json_value() -> impl Strategy<Value = serde_json::Value> {
        let leaf = prop_oneof![
            Just(serde_json::Value::Null),
            any::<bool>().prop_map(serde_json::Value::from),
            any::<i64>().prop_map(serde_json::Value::from),
            // Includes quotes, backslashes, brackets and non-ASCII to stress the string tracker.
            "[ -~\u{e9}\u{2728}]{0,24}".prop_map(serde_json::Value::from),
        ];
        leaf.prop_recursive(4, 32, 4, |inner| {
            prop_oneof![
                prop::collection::vec(inner.clone(), 0..4).prop_map(serde_json::Value::from),
                prop::collection::btree_map("[a-z\"{}]{0,6}", inner, 0..4)
                    .prop_map(|m| serde_json::Value::Object(m.into_iter().collect())),
            ]
        })
    }

    proptest! {
        #[test]
        fn roundtrips_arbitrary_objects_at_any_chunk_size(
            objects in prop::collection::vec(prop::collection::btree_map("[a-z]{1,5}", json_value(), 0..4), 0..6),
            chunk in 1usize..40,
            pretty in any::<bool>(),
        ) {
            let values: Vec<serde_json::Value> = objects
                .into_iter()
                .map(|m| serde_json::Value::Object(m.into_iter().collect()))
                .collect();
            let text = if pretty {
                serde_json::to_string_pretty(&values).unwrap()
            } else {
                serde_json::to_string(&values).unwrap()
            };
            let mut stream = ArrayStream::new(ChunkReader { data: text.as_bytes(), chunk });
            let mut parsed = Vec::new();
            while let Some(bytes) = stream.next_element().unwrap() {
                parsed.push(serde_json::from_slice::<serde_json::Value>(&bytes).unwrap());
            }
            prop_assert_eq!(parsed, values);
        }

        #[test]
        fn arbitrary_bytes_never_panic(bytes in prop::collection::vec(any::<u8>(), 0..256), chunk in 1usize..16) {
            let mut stream = ArrayStream::new(ChunkReader { data: &bytes, chunk });
            for _ in 0..300 {
                match stream.next_element() {
                    Ok(Some(_)) => {}
                    Ok(None) | Err(_) => break,
                }
            }
        }
    }
}
