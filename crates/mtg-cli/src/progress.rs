//! Short progress lines for the person watching the terminal.

use std::fmt::Display;
use std::io::Write;
use std::sync::{Arc, Mutex};

/// Where progress lines go; cheap to clone and safe to share between threads.
#[derive(Clone)]
pub struct Progress {
    sink: Arc<Mutex<Box<dyn Write + Send>>>,
}

/// The text a [`Progress::buffer`] collected.
#[derive(Clone, Default)]
pub struct Captured(Arc<Mutex<Vec<u8>>>);

impl Captured {
    /// Everything written so far.
    pub fn text(&self) -> String {
        self.0
            .lock()
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .unwrap_or_default()
    }
}

struct CaptureWriter(Captured);

impl Write for CaptureWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if let Ok(mut inner) = (self.0).0.lock() {
            inner.extend_from_slice(bytes);
        }
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Progress {
    /// Progress lines on the standard error stream.
    pub fn stderr() -> Self {
        Self::to(std::io::stderr())
    }

    /// Progress lines to any writer.
    pub fn to(sink: impl Write + Send + 'static) -> Self {
        Self {
            sink: Arc::new(Mutex::new(Box::new(sink))),
        }
    }

    /// Progress lines collected in memory, for tests.
    pub fn buffer() -> (Self, Captured) {
        let captured = Captured::default();
        (Self::to(CaptureWriter(captured.clone())), captured)
    }

    /// Writes one line; a sink that cannot be written to is not worth failing a command for.
    pub fn line(&self, text: impl Display) {
        if let Ok(mut sink) = self.sink.lock() {
            let _ = writeln!(sink, "{text}");
        }
    }
}

impl std::fmt::Debug for Progress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Progress")
    }
}
