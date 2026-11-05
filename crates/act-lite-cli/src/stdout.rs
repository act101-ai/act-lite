//! The one place `act-lite` writes to its own stdout.
//!
//! A reader that closes the pipe early (`act-lite query … | head`) makes a
//! write return `ErrorKind::BrokenPipe`. `println!` turns that into a panic.
//! Here it sets a process-wide flag, discards that write and every later
//! stdout write, and lets the command run to completion. `main` then picks
//! the exit code: the command's own non-zero code first, else
//! [`BROKEN_PIPE_EXIT`] when the flag is set, else 0. Any other write error
//! stays loud.

use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};

use tokio::io::AsyncWrite;
use tokio::sync::Notify;

/// 128 + SIGPIPE (13): what a Unix shell reports under `pipefail` when the
/// reader of a pipe leaves.
pub const BROKEN_PIPE_EXIT: i32 = 141;

static CLOSED: AtomicBool = AtomicBool::new(false);

/// Record that act-lite's stdout was closed by its reader.
pub(crate) fn mark_closed() {
    CLOSED.store(true, Ordering::SeqCst);
}

/// True once a stdout write hit a closed pipe (or [`mark_closed`] was called).
pub fn is_closed() -> bool {
    CLOSED.load(Ordering::SeqCst)
}

/// Run one write against `w` and classify the result: `BrokenPipe` sets the
/// flag and is reported as success; any other error is returned.
fn write_classified(
    w: &mut dyn io::Write,
    write: impl FnOnce(&mut dyn io::Write) -> io::Result<()>,
) -> io::Result<()> {
    match write(w).and_then(|()| w.flush()) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => {
            mark_closed();
            Ok(())
        }
        Err(e) => Err(e),
    }
}

/// Fallible stdout write for callers that report errors themselves. After
/// the pipe closes the write is skipped. A closed pipe sets the flag and
/// returns `Ok`; any other error is returned.
pub fn try_with_stdout(write: impl FnOnce(&mut dyn io::Write) -> io::Result<()>) -> io::Result<()> {
    if is_closed() {
        return Ok(());
    }
    let stdout = io::stdout();
    let mut lock = stdout.lock();
    write_classified(&mut lock, write)
}

/// The close side of a [`WatchedWriter`]: a one-shot signal with its own
/// closed flag. A client closing the pipe is a normal end of session, so it
/// never sets the process-wide flag of [`try_with_stdout`].
pub struct CloseSignal {
    closed: AtomicBool,
    notify: Notify,
}

impl CloseSignal {
    fn fire(&self) {
        if !self.closed.swap(true, Ordering::SeqCst) {
            self.notify.notify_one();
        }
    }

    /// True once the watched writer saw `BrokenPipe`.
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    /// Resolves when the pipe closes. A close that already happened resolves
    /// at once: the notification holds its permit until a waiter takes it.
    pub async fn wait(&self) {
        self.notify.notified().await
    }
}

/// A tokio writer whose `BrokenPipe` ends the session instead of failing it:
/// the failing call and every later one are discarded (returning `Ok`), and
/// the [`CloseSignal`] fires once. Other errors pass through.
pub struct WatchedWriter<W> {
    inner: W,
    closed: bool,
    signal: Arc<CloseSignal>,
}

/// Wrap `inner` for watching, with the [`CloseSignal`] that fires when the
/// pipe closes.
pub fn watched<W: AsyncWrite + Unpin>(inner: W) -> (WatchedWriter<W>, Arc<CloseSignal>) {
    let signal = Arc::new(CloseSignal {
        closed: AtomicBool::new(false),
        notify: Notify::new(),
    });
    (
        WatchedWriter {
            inner,
            closed: false,
            signal: Arc::clone(&signal),
        },
        signal,
    )
}

impl<W: AsyncWrite + Unpin> WatchedWriter<W> {
    fn close(&mut self) {
        if !self.closed {
            self.closed = true;
            self.signal.fire();
        }
    }
}

impl<W: AsyncWrite + Unpin> AsyncWrite for WatchedWriter<W> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        if self.closed {
            return Poll::Ready(Ok(buf.len()));
        }
        match Pin::new(&mut self.inner).poll_write(cx, buf) {
            Poll::Ready(Err(e)) if e.kind() == io::ErrorKind::BrokenPipe => {
                self.close();
                Poll::Ready(Ok(buf.len()))
            }
            other => other,
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if self.closed {
            return Poll::Ready(Ok(()));
        }
        match Pin::new(&mut self.inner).poll_flush(cx) {
            Poll::Ready(Err(e)) if e.kind() == io::ErrorKind::BrokenPipe => {
                self.close();
                Poll::Ready(Ok(()))
            }
            other => other,
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if self.closed {
            return Poll::Ready(Ok(()));
        }
        match Pin::new(&mut self.inner).poll_shutdown(cx) {
            Poll::Ready(Err(e)) if e.kind() == io::ErrorKind::BrokenPipe => {
                self.close();
                Poll::Ready(Ok(()))
            }
            other => other,
        }
    }
}

#[cfg(test)]
fn reset_closed() {
    CLOSED.store(false, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use tokio::io::AsyncWriteExt;

    /// Serializes the tests that touch the process-wide flag.
    static FLAG: std::sync::Mutex<()> = std::sync::Mutex::new(());

    struct Failing(io::ErrorKind);
    impl io::Write for Failing {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(self.0.into())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn try_with_stdout_absorbs_broken_pipe_and_sets_flag() {
        let _guard = FLAG.lock().unwrap_or_else(|e| e.into_inner());
        reset_closed();
        write_classified(&mut Failing(io::ErrorKind::BrokenPipe), |w| {
            w.write_all(b"x")
        })
        .unwrap_or_else(|e| panic!("a closed pipe must be absorbed, got {e}"));
        let closed = is_closed();
        reset_closed();
        assert!(closed, "BrokenPipe did not set the flag");
    }

    #[test]
    fn try_with_stdout_returns_other_errors() {
        let _guard = FLAG.lock().unwrap_or_else(|e| e.into_inner());
        reset_closed();
        let err = write_classified(&mut Failing(io::ErrorKind::StorageFull), |w| {
            w.write_all(b"x")
        })
        .unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::StorageFull);
        let closed = is_closed();
        reset_closed();
        assert!(!closed, "a non-BrokenPipe error set the flag");
    }

    #[test]
    fn flag_discards_after_close() {
        let _guard = FLAG.lock().unwrap_or_else(|e| e.into_inner());
        reset_closed();
        mark_closed();
        let mut called = false;
        try_with_stdout(|_| {
            called = true;
            Ok(())
        })
        .unwrap();
        let closed = is_closed();
        reset_closed();
        assert!(closed, "flag not set");
        assert!(!called, "closure ran after close");
    }

    /// Inner writer whose write and/or flush fail with a chosen kind.
    struct Faulty {
        write_err: Option<io::ErrorKind>,
        flush_err: Option<io::ErrorKind>,
        writes: usize,
    }

    impl AsyncWrite for Faulty {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            self.writes += 1;
            match self.write_err {
                Some(kind) => Poll::Ready(Err(kind.into())),
                None => Poll::Ready(Ok(buf.len())),
            }
        }
        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
            match self.flush_err {
                Some(kind) => Poll::Ready(Err(kind.into())),
                None => Poll::Ready(Ok(())),
            }
        }
        fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    #[tokio::test]
    async fn watched_writer_swallows_broken_pipe_and_notifies() {
        let (mut w, closed) = watched(Faulty {
            write_err: Some(io::ErrorKind::BrokenPipe),
            flush_err: None,
            writes: 0,
        });
        w.write_all(b"hello")
            .await
            .unwrap_or_else(|e| panic!("a closed pipe must be swallowed, got {e}"));
        tokio::time::timeout(Duration::from_secs(1), closed.notify.notified())
            .await
            .expect("no notification");
        w.write_all(b"again").await.unwrap();
        w.flush().await.unwrap();
    }

    #[tokio::test]
    async fn watched_writer_flush_broken_pipe_notifies() {
        let (mut w, closed) = watched(Faulty {
            write_err: None,
            flush_err: Some(io::ErrorKind::BrokenPipe),
            writes: 0,
        });
        w.write_all(b"hello").await.unwrap();
        w.flush()
            .await
            .unwrap_or_else(|e| panic!("a closed pipe on flush must be swallowed, got {e}"));
        tokio::time::timeout(Duration::from_secs(1), closed.notify.notified())
            .await
            .expect("no notification");
        let before = w.inner.writes;
        w.write_all(b"again").await.unwrap();
        assert_eq!(w.inner.writes, before, "a write reached the closed inner");
    }

    #[tokio::test]
    async fn watched_writer_passes_other_errors_through() {
        let (mut w, closed) = watched(Faulty {
            write_err: Some(io::ErrorKind::PermissionDenied),
            flush_err: None,
            writes: 0,
        });
        let err = w.write_all(b"hello").await.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
        assert!(
            tokio::time::timeout(Duration::from_millis(50), closed.notify.notified())
                .await
                .is_err(),
            "notified on a non-BrokenPipe error"
        );
    }
}
