use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::mpsc;
use std::thread;

use crate::Secrets;
use crate::error::SecretsError;
use crate::redact::MIN_REDACT_LEN;

/// Result of running a command with secrets injected: its exit code. The
/// redacted output is streamed to the caller and not kept.
#[derive(Debug)]
pub struct RunOutcome {
    pub code: i32,
    /// True when this process's stdout was closed by its reader mid-run.
    pub stdout_closed: bool,
}

/// A writer that treats a closed pipe as "the reader left" rather than a
/// failure. `BrokenPipe` from `write` or `flush` sets `closed` and is reported
/// as success; once closed, every write and flush is discarded.
struct ClosableSink<W: Write> {
    inner: W,
    closed: bool,
    /// `strict` sinks pass every error through (used for stderr).
    strict: bool,
}

impl<W: Write> ClosableSink<W> {
    fn new(inner: W) -> Self {
        Self {
            inner,
            closed: false,
            strict: false,
        }
    }

    fn strict(inner: W) -> Self {
        Self {
            inner,
            closed: false,
            strict: true,
        }
    }
}

impl<W: Write> Write for ClosableSink<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self.closed {
            return Ok(buf.len());
        }
        match self.inner.write(buf) {
            Err(e) if !self.strict && e.kind() == io::ErrorKind::BrokenPipe => {
                self.closed = true;
                Ok(buf.len())
            }
            other => other,
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.closed {
            return Ok(());
        }
        match self.inner.flush() {
            Err(e) if !self.strict && e.kind() == io::ErrorKind::BrokenPipe => {
                self.closed = true;
                Ok(())
            }
            other => other,
        }
    }
}

/// Exit code of a finished child: its own code; 141 when a signal 13
/// (SIGPIPE) killed it (Unix); otherwise 1.
fn exit_code(status: std::process::ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if status.signal() == Some(13) {
            return 141;
        }
    }
    1
}

/// Run `argv` as a child process with every secret injected into its
/// environment. Stdin is inherited. Stdout/stderr are streamed live to this
/// process's stdout/stderr and redacted on the way: bytes are flushed as they
/// arrive, except a tail that could be the start of a stored secret, which
/// waits for more bytes or end of stream. Both streams are written in the order their bytes arrive, and a stored
/// value contiguous in that merged order is redacted too, so a value split
/// across stdout and stderr is redacted when they are merged.
pub fn run_command(secrets: &Secrets, argv: &[String]) -> Result<RunOutcome, SecretsError> {
    run_with(
        secrets,
        argv,
        ClosableSink::new(io::stdout()),
        ClosableSink::strict(io::stderr()),
    )
}

/// `run_command` with the child's redacted stdout and stderr written to `out`
/// and `err`.
fn run_with<O: Write + Send, E: Write + Send>(
    secrets: &Secrets,
    argv: &[String],
    out: ClosableSink<O>,
    err: ClosableSink<E>,
) -> Result<RunOutcome, SecretsError> {
    let (program, rest) = argv.split_first().ok_or(SecretsError::NoCommand)?;

    let mut child = Command::new(program)
        .args(rest)
        .envs(secrets.env_pairs())
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| SecretsError::Spawn {
            program: program.clone(),
            source,
        })?;

    let values = secrets.values();
    let child_out = child.stdout.take().expect("stdout was piped");
    let child_err = child.stderr.take().expect("stderr was piped");

    // Drain both pipes concurrently to avoid a deadlock when the child fills one
    // while we block on the other. A scoped thread lets the pump borrow `values`.
    let merged = Mutex::new(Merged::new(out, err));
    thread::scope(|scope| -> io::Result<()> {
        let out_pump = scope.spawn(|| pump(child_out, Stream::Stdout, &values, &merged));
        pump(child_err, Stream::Stderr, &values, &merged)?;
        out_pump.join().expect("stdout pump thread panicked")
    })
    .map_err(|source| SecretsError::Io {
        action: "stream output of",
        path: PathBuf::from(program),
        source,
    })?;

    let status = child.wait().map_err(|source| SecretsError::Io {
        action: "wait for",
        path: PathBuf::from(program),
        source,
    })?;
    let merged = merged.into_inner().expect("a pump thread panicked");
    Ok(RunOutcome {
        code: exit_code(status),
        stdout_closed: merged.out.closed,
    })
}

/// One of the child's two output streams.
#[derive(Clone, Copy, PartialEq)]
enum Stream {
    Stdout,
    Stderr,
}

/// The stored values long enough to redact, as bytes.
fn redactable(values: &[String]) -> Vec<&[u8]> {
    values
        .iter()
        .map(String::as_bytes)
        .filter(|v| v.len() >= MIN_REDACT_LEN)
        .collect()
}

/// One arrived byte that is not yet written.
struct Byte {
    byte: u8,
    stream: Stream,
    /// Already sent to the sink; retained only as masked match history.
    written: bool,
    /// Its stream has released it.
    released: bool,
    /// Part of a stored value.
    masked: bool,
    /// First byte of a stored value: `***` is written here.
    starts: bool,
}

/// The single writer for both streams. Bytes enter in arrival order
/// (`arrive`) and are written in queue order, each to its own stream, once
/// its stream has released it (`release`) and no stored value can still start
/// at it. A stream's unreleased bytes move behind the other stream's released
/// bytes once together they can no longer start a stored value, so a held
/// tail holds back only its own stream. A stored value is masked when it is
/// contiguous in queue order (both streams merged) or in one stream; each
/// stream writes `***` where a value starts and where its part of a value
/// starts.
struct Merged<O: Write, E: Write> {
    out: ClosableSink<O>,
    err: ClosableSink<E>,
    /// Pending bytes plus a short suffix already written and masked, in order.
    queue: VecDeque<Byte>,
    /// Per stream: the last byte written was masked.
    masking: [bool; 2],
    /// Per stream: written bytes that end in an incomplete UTF-8 character.
    carry: [Vec<u8>; 2],
    /// Streams whose pump has not ended.
    open: usize,
    /// Bytes emitted before their pump released them, awaiting its acknowledgement.
    forced: [usize; 2],
    /// Masked bytes already sent to each stream, for matches across later chunks.
    history: [Vec<u8>; 2],
}

impl<O: Write, E: Write> Merged<O, E> {
    fn new(out: ClosableSink<O>, err: ClosableSink<E>) -> Self {
        Self {
            out,
            err,
            queue: VecDeque::new(),
            masking: [false; 2],
            carry: Default::default(),
            open: 2,
            forced: [0; 2],
            history: Default::default(),
        }
    }

    fn closed(&self, stream: Stream) -> bool {
        match stream {
            Stream::Stdout => self.out.closed,
            Stream::Stderr => self.err.closed,
        }
    }

    /// The queued bytes from index `at` on start with `prefix`.
    fn starts_with(&self, at: usize, prefix: &[u8]) -> bool {
        self.queue.len() >= at + prefix.len()
            && self
                .queue
                .range(at..at + prefix.len())
                .map(|b| b.byte)
                .eq(prefix.iter().copied())
    }

    /// Mark the bytes at queue indexes `at` as one stored value.
    fn mask(&mut self, at: impl Iterator<Item = usize>) {
        for (i, p) in at.enumerate() {
            let b = &mut self.queue[p];
            if i == 0 {
                b.starts |= !b.masked;
            } else {
                b.starts = false;
            }
            b.masked = true;
        }
    }

    /// Mask every stored value contiguous in the queue that ends after index
    /// `old` and starts at or after index `from`.
    fn mask_queued(&mut self, from: usize, old: usize, values: &[String]) {
        let vals = redactable(values);
        let longest = vals.iter().map(|v| v.len()).max().unwrap_or(0);
        for p in old.saturating_sub(longest).max(from)..self.queue.len() {
            for v in &vals {
                if p + v.len() > old && self.starts_with(p, v) {
                    self.mask(p..p + v.len());
                }
            }
        }
    }

    /// Mark a value completed in one stream even when the other stream's
    /// bytes separate its parts in queue order.
    fn mask_stream(&mut self, stream: Stream, old: usize, values: &[String]) {
        let vals = redactable(values);
        if vals.is_empty() {
            return;
        }
        let mut bytes: Vec<(u8, Option<usize>)> = self.history[stream as usize]
            .iter()
            .map(|&b| (b, None))
            .collect();
        bytes.extend(
            self.queue
                .iter()
                .enumerate()
                .filter(|(_, b)| b.stream == stream && !b.written)
                .map(|(i, b)| (b.byte, Some(i))),
        );
        for v in vals {
            for i in 0..bytes.len().saturating_sub(v.len() - 1) {
                let hit = &bytes[i..i + v.len()];
                if hit.iter().any(|(_, p)| p.is_some_and(|p| p >= old))
                    && hit.iter().map(|(b, _)| *b).eq(v.iter().copied())
                {
                    self.mask(hit.iter().filter_map(|(_, p)| *p));
                }
            }
        }
    }

    /// Queue `bytes` just read from `stream`, mask every stored value they
    /// complete in queue order, and write what is ready.
    fn arrive(&mut self, stream: Stream, bytes: &[u8], values: &[String]) -> io::Result<()> {
        let old = self.queue.len();
        self.queue.extend(bytes.iter().map(|&byte| Byte {
            byte,
            stream,
            written: false,
            released: false,
            masked: false,
            starts: false,
        }));
        self.mask_queued(0, old, values);
        self.mask_stream(stream, old, values);
        self.write(values, false)
    }

    /// Mark the next `bytes.len()` queued bytes of `stream` released, mask
    /// every stored value contiguous in them, and write what is ready.
    fn release(&mut self, stream: Stream, bytes: &[u8], values: &[String]) -> io::Result<()> {
        let acknowledged = self.forced[stream as usize].min(bytes.len());
        self.forced[stream as usize] -= acknowledged;
        let bytes = &bytes[acknowledged..];
        let at: Vec<usize> = (0..self.queue.len())
            .filter(|&p| self.queue[p].stream == stream && !self.queue[p].released)
            .take(bytes.len())
            .collect();
        assert_eq!(at.len(), bytes.len(), "released bytes have arrived");
        for &p in &at {
            self.queue[p].released = true;
        }
        for v in redactable(values) {
            for i in 0..bytes.len() {
                if bytes[i..].starts_with(v) {
                    self.mask(at[i..i + v.len()].iter().copied());
                }
            }
        }
        self.write(values, true)
    }

    /// Index of the first unreleased queued byte, or the queue length.
    fn unreleased(&self) -> usize {
        self.queue
            .iter()
            .position(|b| !b.released)
            .unwrap_or(self.queue.len())
    }

    /// Let the other stream's released bytes pass a held tail: when the
    /// queued bytes from no index up to the first unreleased byte are a proper
    /// prefix of a stored value, move that byte's stream's unreleased bytes to
    /// the end of the queue, and mask every value the new order makes
    /// contiguous. While such a prefix exists, both streams wait.
    fn pass_held_tail(&mut self, values: &[String]) {
        let u = self.unreleased();
        let Some(held) = self.queue.get(u).map(|b| b.stream) else {
            return;
        };
        let passes = self
            .queue
            .range(u..)
            .any(|b| b.stream != held && b.released);
        let end = self.queue.len();
        let vals = redactable(values);
        let longest = vals.iter().map(|v| v.len()).max().unwrap_or(0);
        let could_start = (end.saturating_sub(longest)..=u).any(|p| {
            vals.iter()
                .any(|v| v.len() > end - p && self.starts_with(p, &v[..end - p]))
        });
        if !passes || could_start {
            return;
        }
        let tail: Vec<Byte> = self.queue.drain(u..).collect();
        let (moved, kept): (Vec<Byte>, Vec<Byte>) = tail
            .into_iter()
            .partition(|b| b.stream == held && !b.released);
        self.queue.extend(kept);
        self.queue.extend(moved);
        self.mask_queued(u.saturating_sub(longest), u, values);
    }

    /// Index where writing stops: the first unreleased byte, moved back to
    /// the start of a possible value prefix. A released prefix can be emitted
    /// when the held tail in the same stream repeats it:
    /// any future completion then also matches the held tail.
    /// Nothing stops once no stream is open.
    fn ready(&self, values: &[String]) -> usize {
        if self.open == 0 {
            return self.queue.len();
        }
        let u = self.unreleased();
        let vals = redactable(values);
        let longest = vals.iter().map(|v| v.len()).max().unwrap_or(0);
        let held_stream = self.queue.get(u).map(|b| b.stream);
        let held = self
            .queue
            .range(u..)
            .take_while(|b| !b.released && Some(b.stream) == held_stream)
            .count();
        (u.saturating_sub(longest)..u)
            .find(|&p| {
                vals.iter()
                    .any(|v| v.len() > u - p && self.starts_with(p, &v[..u - p]))
                    && !(held >= u - p
                        && self
                            .queue
                            .range(p..u)
                            .all(|b| Some(b.stream) == held_stream)
                        && self
                            .queue
                            .range(p..u)
                            .map(|b| b.byte)
                            .eq(self.queue.range(u..u + (u - p)).map(|b| b.byte)))
            })
            .unwrap_or(u)
    }

    /// Write ready bytes, then mask and emit the oldest held prefix until no
    /// more than one proper secret prefix remains unwritten.
    fn write(&mut self, values: &[String], enforce_bound: bool) -> io::Result<()> {
        let max_hold = redactable(values)
            .iter()
            .map(|v| v.len())
            .max()
            .unwrap_or(0)
            .saturating_sub(1);
        let max_history = if self.open == 0 { 0 } else { max_hold };
        loop {
            self.pass_held_tail(values);
            let ready = self.ready(values);
            self.write_through(ready, max_history)?;
            if !enforce_bound || self.queue.iter().filter(|b| !b.written).count() <= max_hold {
                return Ok(());
            }
            let p = self.queue.iter().position(|b| !b.written).unwrap();
            let b = &mut self.queue[p];
            if !b.released {
                self.forced[b.stream as usize] += 1;
                b.released = true;
            }
            if max_hold != 0 {
                b.masked = true;
                b.starts = false;
            }
            self.write_through(p + 1, max_history)?;
        }
    }

    /// Emit bytes through `ready` and retain only a bounded masked suffix as
    /// already-written history for matches that span the two streams.
    fn write_through(&mut self, ready: usize, max_history: usize) -> io::Result<()> {
        let mut piece = Vec::new();
        let mut current = None;
        let drained: Vec<Byte> = self.queue.drain(..ready).collect();
        for b in drained.iter().filter(|b| !b.written) {
            if current != Some(b.stream) {
                if let Some(stream) = current {
                    self.emit(stream, &mut piece)?;
                }
                current = Some(b.stream);
            }
            let masking = &mut self.masking[b.stream as usize];
            if !b.masked {
                piece.push(b.byte);
            } else if b.starts || !*masking {
                piece.extend_from_slice(b"***");
            }
            *masking = b.masked;
            let history = &mut self.history[b.stream as usize];
            if b.masked {
                history.push(b.byte);
                let excess = history.len().saturating_sub(max_history);
                history.drain(..excess);
            } else {
                history.clear();
            }
        }
        if let Some(stream) = current {
            self.emit(stream, &mut piece)?;
        }
        for b in drained
            .into_iter()
            .rev()
            .take_while(|b| b.masked)
            .take(max_history)
            .collect::<Vec<_>>()
            .into_iter()
        {
            let mut b = b;
            b.written = true;
            self.queue.push_front(b);
        }
        Ok(())
    }

    /// Write `bytes` to `stream`. Bytes are decoded lossily so non-UTF-8
    /// output is tolerated; an incomplete character at the end waits for the
    /// stream's next bytes while a stream is open.
    fn emit(&mut self, stream: Stream, bytes: &mut Vec<u8>) -> io::Result<()> {
        let carry = &mut self.carry[stream as usize];
        carry.append(bytes);
        let keep = if self.open == 0 {
            0
        } else {
            incomplete_tail(carry)
        };
        let done: Vec<u8> = carry.drain(..carry.len() - keep).collect();
        if done.is_empty() {
            return Ok(());
        }
        let text = String::from_utf8_lossy(&done);
        let sink: &mut dyn Write = match stream {
            Stream::Stdout => &mut self.out,
            Stream::Stderr => &mut self.err,
        };
        sink.write_all(text.as_bytes())?;
        sink.flush()
    }

    /// End `stream`: release `rest`, its last bytes. Once no stream is open,
    /// everything left is written.
    fn finish(&mut self, stream: Stream, rest: &[u8], values: &[String]) -> io::Result<()> {
        self.open -= 1;
        self.release(stream, rest, values)?;
        if self.open == 0 {
            for stream in [Stream::Stdout, Stream::Stderr] {
                self.emit(stream, &mut Vec::new())?;
            }
        }
        Ok(())
    }
}

/// Length of an incomplete UTF-8 character at the end of `bytes`.
fn incomplete_tail(bytes: &[u8]) -> usize {
    for back in 1..=bytes.len().min(3) {
        let b = bytes[bytes.len() - back];
        if b & 0xC0 != 0x80 {
            let need = match b {
                0xC0..=0xDF => 2,
                0xE0..=0xEF => 3,
                0xF0..=0xF7 => 4,
                _ => 1,
            };
            return if need > back { back } else { 0 };
        }
    }
    0
}

/// Index where a release stops: the start of the longest suffix of
/// `pending` that is a proper prefix of a redactable value. Complete values
/// have already been masked by `Merged::arrive`, so they need not stay pending.
fn idle_cut(pending: &[u8], values: &[String]) -> usize {
    let vals = redactable(values);
    let longest = vals.iter().map(|v| v.len()).max().unwrap_or(0);
    (pending.len().saturating_sub(longest)..pending.len())
        .find(|&i| {
            let tail = &pending[i..];
            vals.iter()
                .any(|v| v.len() > tail.len() && v.starts_with(tail))
        })
        .unwrap_or(pending.len())
}

/// Read `reader` on a helper thread; queue each chunk in `merged` as it
/// arrives, and release it as `stream` up to `idle_cut`; the rest releases
/// with later bytes or at end of stream.
fn pump<R: Read + Send + 'static, O: Write, E: Write>(
    mut reader: R,
    stream: Stream,
    values: &[String],
    merged: &Mutex<Merged<O, E>>,
) -> io::Result<()> {
    let (tx, rx) = mpsc::channel::<io::Result<Vec<u8>>>();
    thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            let msg = match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => Ok(buf[..n].to_vec()),
                Err(e) => Err(e),
            };
            let is_err = msg.is_err();
            if tx.send(msg).is_err() || is_err {
                break;
            }
        }
    });

    let mut pending: Vec<u8> = Vec::new();

    loop {
        match rx.recv() {
            Ok(Ok(chunk)) => {
                pending.extend_from_slice(&chunk);
                let ready: Vec<u8> = pending.drain(..idle_cut(&pending, values)).collect();
                let mut merged = merged.lock().expect("a pump thread panicked");
                merged.arrive(stream, &chunk, values)?;
                merged.release(stream, &ready, values)?;
                if merged.closed(stream) {
                    break;
                }
            }
            Ok(Err(e)) => return Err(e),
            Err(mpsc::RecvError) => break,
        }
    }
    merged
        .lock()
        .expect("a pump thread panicked")
        .finish(stream, &pending, values)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    /// One tempdir holding a git-rooted project (`proj/`) and a private
    /// secrets dir (`secrets/`). Returns (tempdir, root, secrets).
    fn sandbox() -> (tempfile::TempDir, std::path::PathBuf, Secrets) {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("proj");
        std::fs::create_dir_all(root.join(".git")).unwrap();
        let root = root.canonicalize().unwrap();
        let secrets = Secrets::open_in(&root, &tmp.path().join("secrets")).unwrap();
        (tmp, root, secrets)
    }

    fn text(bytes: &[u8]) -> String {
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    #[cfg(unix)]
    fn sh(script: &str) -> Vec<String> {
        vec!["sh".into(), "-c".into(), script.into()]
    }

    #[cfg(unix)]
    #[test]
    fn injects_secret_and_redacts_it_from_output() {
        let (_tmp, _root, mut secrets) = sandbox();
        secrets.add("MY_TOKEN", "supersecretvalue").unwrap();

        // The child echoes the secret; redaction must scrub it on the way back.
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let outcome = run_with(
            &secrets,
            &sh("echo $MY_TOKEN"),
            ClosableSink::new(&mut out),
            ClosableSink::strict(&mut err),
        )
        .unwrap();
        assert_eq!(outcome.code, 0);
        let stdout = text(&out);
        assert!(stdout.contains("***"));
        assert!(!stdout.contains("supersecretvalue"));
    }

    #[cfg(unix)]
    #[test]
    fn redacts_secret_from_stderr() {
        let (_tmp, _root, mut secrets) = sandbox();
        secrets.add("MY_TOKEN", "supersecretvalue").unwrap();

        // The child echoes the secret to stderr; redaction must scrub it there too.
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let outcome = run_with(
            &secrets,
            &sh("echo $MY_TOKEN >&2"),
            ClosableSink::new(&mut out),
            ClosableSink::strict(&mut err),
        )
        .unwrap();
        assert_eq!(outcome.code, 0);
        let stderr = text(&err);
        assert!(stderr.contains("***"));
        assert!(!stderr.contains("supersecretvalue"));
    }

    #[cfg(unix)]
    #[test]
    fn a_prompt_without_newline_is_delivered_before_the_child_exits() {
        // The child prints a prompt with no newline, then sleeps past the idle
        // flush, then exits. The captured stdout must contain the prompt either
        // way; the assertion that matters is timing, checked by the `Probe`
        // writer, which records when the first byte reaches it.
        use std::sync::{Arc, Mutex};
        use std::time::Instant;

        #[derive(Clone)]
        struct Probe(Arc<Mutex<Option<Instant>>>, Arc<Mutex<Vec<u8>>>);
        impl std::io::Write for Probe {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                let mut first = self.0.lock().unwrap();
                if first.is_none() && !buf.is_empty() {
                    *first = Some(Instant::now());
                }
                self.1.lock().unwrap().extend_from_slice(buf);
                Ok(buf.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let (_tmp, _root, secrets) = sandbox();
        let mut child = std::process::Command::new("sh")
            .args(["-c", "printf 'Password: '; sleep 1"])
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let out = child.stdout.take().unwrap();
        let probe = Probe(Arc::new(Mutex::new(None)), Arc::default());
        let started = Instant::now();
        let merged = stdout_only(ClosableSink::new(probe.clone()));
        pump(out, Stream::Stdout, &secrets.values(), &merged).unwrap();
        child.wait().unwrap();

        assert_eq!(text(&probe.1.lock().unwrap()), "Password: ");
        let first = probe.0.lock().unwrap().expect("prompt was written");
        assert!(
            first.duration_since(started) < std::time::Duration::from_millis(700),
            "prompt reached the sink only after the child exited"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_secret_split_across_chunks_on_one_line_is_still_redacted() {
        let (_tmp, _root, mut secrets) = sandbox();
        secrets.add("T", "supersecretvalue").unwrap();
        // Two writes, no newline between them, well inside the idle window.
        let (mut out, mut err) = (Vec::new(), Vec::new());
        run_with(
            &secrets,
            &sh("printf 'super'; printf 'secretvalue\\n'"),
            ClosableSink::new(&mut out),
            ClosableSink::strict(&mut err),
        )
        .unwrap();
        assert_eq!(text(&out), "***\n");
    }

    #[cfg(unix)]
    fn rss_kib() -> u64 {
        let out = std::process::Command::new("ps")
            .args(["-o", "rss=", "-p", &std::process::id().to_string()])
            .output()
            .unwrap();
        String::from_utf8(out.stdout)
            .unwrap()
            .trim()
            .parse()
            .unwrap()
    }

    #[cfg(unix)]
    #[test]
    fn run_retains_no_output() {
        let before = rss_kib();
        let mut m = Merged::new(ClosableSink::new(io::sink()), ClosableSink::new(io::sink()));
        let chunk = [b'x'; 8192];
        for _ in 0..(32 * 1024 * 1024 / chunk.len()) {
            for stream in [Stream::Stdout, Stream::Stderr] {
                m.arrive(stream, &chunk, &[]).unwrap();
                m.release(stream, &chunk, &[]).unwrap();
            }
        }
        let growth = rss_kib().saturating_sub(before);
        assert!(growth < 32768, "resident size grew by {growth} KiB");
        drop(m);
    }

    fn both() -> Merged<Vec<u8>, Vec<u8>> {
        Merged::new(ClosableSink::new(Vec::new()), ClosableSink::new(Vec::new()))
    }

    #[test]
    fn a_value_contiguous_in_arrival_order_is_masked_in_both_streams() {
        let values = vec!["supersecretvalue".to_string()];
        let mut m = both();
        m.arrive(Stream::Stdout, b"a super", &values).unwrap();
        m.release(Stream::Stdout, b"a ", &values).unwrap();
        assert_eq!(text(&m.out.inner), "a ");
        m.arrive(Stream::Stderr, b"secretvalue\n", &values).unwrap();
        m.release(Stream::Stderr, b"secretvalue\n", &values)
            .unwrap();
        // `super` + `secretvalue\n` can no longer start a value: stderr passes
        // stdout's held `super`.
        assert_eq!(text(&m.err.inner), "***\n");
        m.finish(Stream::Stdout, b"super", &values).unwrap();
        m.finish(Stream::Stderr, b"", &values).unwrap();
        assert_eq!(text(&m.out.inner), "a ***");
        assert_eq!(text(&m.err.inner), "***\n");
    }

    #[test]
    fn bytes_that_cannot_form_a_value_are_written_unchanged() {
        let values = vec!["supersecretvalue".to_string()];
        let mut m = both();
        m.arrive(Stream::Stdout, b"sups", &values).unwrap();
        m.release(Stream::Stdout, b"sup", &values).unwrap();
        // `sup` waits: stderr may pass the held `s` and continue it.
        assert_eq!(text(&m.out.inner), "");
        m.arrive(Stream::Stderr, b"xyz\n", &values).unwrap();
        m.release(Stream::Stderr, b"xyz\n", &values).unwrap();
        assert_eq!(text(&m.out.inner), "sup");
        assert_eq!(text(&m.err.inner), "xyz\n");
        m.finish(Stream::Stdout, b"s", &values).unwrap();
        m.finish(Stream::Stderr, b"", &values).unwrap();
        assert_eq!(text(&m.out.inner), "sups");
    }

    #[test]
    fn a_value_formed_where_the_other_stream_passes_a_held_tail_is_masked() {
        let values = vec!["supersecretvalue".to_string()];
        let mut m = both();
        m.arrive(Stream::Stdout, b"sups", &values).unwrap();
        m.release(Stream::Stdout, b"sup", &values).unwrap();
        m.arrive(Stream::Stderr, b"ersecretvalue\n", &values)
            .unwrap();
        m.release(Stream::Stderr, b"ersecretvalue\n", &values)
            .unwrap();
        // Written order is `sup`, `ersecretvalue\n`, `s`: the value is contiguous.
        m.finish(Stream::Stdout, b"s", &values).unwrap();
        m.finish(Stream::Stderr, b"", &values).unwrap();
        assert_eq!(text(&m.out.inner), "***s");
        assert_eq!(text(&m.err.inner), "***\n");
    }

    #[test]
    fn a_flood_behind_a_held_tail_is_written_as_it_arrives() {
        let values = vec!["abcdef123456".to_string()];
        let mut m = both();
        m.arrive(Stream::Stdout, b"progress a", &values).unwrap();
        m.release(Stream::Stdout, b"progress ", &values).unwrap();
        let chunk = [b'x'; 8192];
        for _ in 0..128 {
            m.arrive(Stream::Stderr, &chunk, &values).unwrap();
            m.release(Stream::Stderr, &chunk, &values).unwrap();
            assert!(m.queue.len() <= 1, "{} bytes queued", m.queue.len());
        }
        assert_eq!(text(&m.out.inner), "progress ");
        assert_eq!(m.err.inner.len(), 128 * 8192);
    }

    #[test]
    fn a_value_contiguous_in_one_stream_is_masked_across_the_other() {
        let values = vec!["supersecretvalue".to_string()];
        let mut m = both();
        m.arrive(Stream::Stdout, b"super", &values).unwrap();
        m.arrive(Stream::Stderr, b"warn\n", &values).unwrap();
        m.release(Stream::Stderr, b"warn\n", &values).unwrap();
        m.arrive(Stream::Stdout, b"secretvalue\n", &values).unwrap();
        m.release(Stream::Stdout, b"supersecretvalue\n", &values)
            .unwrap();
        assert_eq!(text(&m.out.inner), "***\n");
        assert_eq!(text(&m.err.inner), "warn\n");
    }

    #[test]
    fn a_character_split_by_the_other_stream_is_written_whole() {
        let mut m = both();
        m.arrive(Stream::Stdout, b"caf\xc3", &[]).unwrap();
        m.arrive(Stream::Stderr, b"x\n", &[]).unwrap();
        m.release(Stream::Stderr, b"x\n", &[]).unwrap();
        m.arrive(Stream::Stdout, b"\xa9\n", &[]).unwrap();
        m.release(Stream::Stdout, b"caf\xc3\xa9\n", &[]).unwrap();
        assert_eq!(text(&m.out.inner), "caf\u{e9}\n");
    }

    #[test]
    fn adjacent_and_nested_values_are_masked() {
        let masked = |values: &[&str], bytes: &[u8]| {
            let values: Vec<String> = values.iter().map(|v| v.to_string()).collect();
            let mut m = both();
            m.arrive(Stream::Stdout, bytes, &values).unwrap();
            m.release(Stream::Stdout, bytes, &values).unwrap();
            text(&m.out.inner)
        };
        let nested = ["abcdef", "abcdefghijkl"];
        assert_eq!(
            masked(&nested, b"x abcdefghijkl y abcdefabcdef z\n"),
            "x *** y ****** z\n"
        );
        assert_eq!(
            masked(&nested, b"x abcdefghijkl y abcdef z\n"),
            "x *** y *** z\n"
        );
        assert_eq!(
            masked(&["supersecret", "supersecret"], b"supersecret\n"),
            "***\n"
        );
        // Values under MIN_REDACT_LEN are never masked.
        assert_eq!(masked(&["true"], b"flag=true\n"), "flag=true\n");
    }

    #[test]
    fn idle_cut_holds_back_only_a_possible_secret_start() {
        let values = vec!["supersecretvalue".to_string(), "short".to_string()];
        // No tail starts a value: everything flushes.
        assert_eq!(idle_cut(b"Password: ", &values), 10);
        // The tail `super` starts a value: it is held back.
        assert_eq!(idle_cut(b"key=super", &values), 4);
        // Values under MIN_REDACT_LEN are never held for.
        assert_eq!(idle_cut(b"key=sho", &values), 7);
        // A complete value crossing the cut is already masked in Merged.
        let values = vec!["abcdefgh".to_string(), "fghijklm".to_string()];
        assert_eq!(idle_cut(b"x abcdefghij", &values), 7);
        let periodic = vec!["aaaaaa".to_string()];
        assert_eq!(idle_cut(&vec![b'a'; 400], &periodic), 395);
        assert_eq!(idle_cut(&vec![b'b'; 400], &periodic), 400);
    }

    #[cfg(unix)]
    #[test]
    fn periodic_secret_streams_masked_bytes_before_child_exit() {
        struct SignalSink(mpsc::Sender<Vec<u8>>);
        impl Write for SignalSink {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                self.0.send(buf.to_vec()).unwrap();
                Ok(buf.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let (_tmp, _root, mut secrets) = sandbox();
        secrets.add("SECA", "aaaaaa").unwrap();
        let (tx, rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            run_with(
                &secrets,
                &sh("printf '%400s' '' | tr ' ' a; sleep 8"),
                ClosableSink::new(SignalSink(tx)),
                ClosableSink::strict(io::sink()),
            )
        });
        let first = rx
            .recv_timeout(Duration::from_secs(6))
            .expect("no safe output while child idle");
        assert_eq!(first, b"***");
        assert_eq!(worker.join().unwrap().unwrap().code, 0);
        assert!(rx.try_iter().all(|chunk| !chunk.contains(&b'a')));

        let (_tmp, _root, mut secrets) = sandbox();
        secrets.add("SECA", "aaaaaa").unwrap();
        let (tx, rx) = mpsc::channel();
        let worker = thread::spawn(move || {
            run_with(
                &secrets,
                &sh("printf '%400s' '' | tr ' ' b; sleep 2"),
                ClosableSink::new(SignalSink(tx)),
                ClosableSink::strict(io::sink()),
            )
        });
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(1))
                .expect("control output did not stream while idle"),
            vec![b'b'; 400]
        );
        assert_eq!(worker.join().unwrap().unwrap().code, 0);
    }

    #[test]
    fn a_complete_value_crossing_the_idle_cut_stays_masked_across_streams() {
        let values = vec!["abcdefgh".to_string(), "fghijklm".to_string()];
        let mut m = both();
        m.arrive(Stream::Stdout, b"abc", &values).unwrap();
        m.arrive(Stream::Stderr, b"X", &values).unwrap();
        m.arrive(Stream::Stdout, b"defghij", &values).unwrap();
        let pending = b"abcdefghij";
        let cut = idle_cut(pending, &values);
        m.release(Stream::Stdout, &pending[..cut], &values).unwrap();
        m.release(Stream::Stderr, b"X", &values).unwrap();
        m.finish(Stream::Stdout, &pending[cut..], &values).unwrap();
        m.finish(Stream::Stderr, b"", &values).unwrap();
        assert_eq!(text(&m.out.inner), "***ij");
        assert_eq!(text(&m.err.inner), "X");
    }

    #[test]
    fn periodic_secret_holds_no_more_than_its_proper_prefix() {
        struct PausingReader {
            sent: bool,
            resume: mpsc::Receiver<()>,
        }
        impl Read for PausingReader {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                if self.sent {
                    self.resume.recv().unwrap();
                    return Ok(0);
                }
                self.sent = true;
                buf[..400].fill(b'a');
                Ok(400)
            }
        }
        struct SignalSink(mpsc::Sender<()>);
        impl Write for SignalSink {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                self.0.send(()).unwrap();
                Ok(buf.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        for secret in ["aaaaaa", "aaaaab"] {
            let (resume_tx, resume_rx) = mpsc::channel();
            let (written_tx, written_rx) = mpsc::channel();
            let merged = Arc::new(Mutex::new(Merged::new(
                ClosableSink::new(SignalSink(written_tx)),
                ClosableSink::strict(io::sink()),
            )));
            let other = merged.clone();
            let values = vec![secret.to_string()];
            let worker = thread::spawn(move || {
                pump(
                    PausingReader {
                        sent: false,
                        resume: resume_rx,
                    },
                    Stream::Stdout,
                    &values,
                    &other,
                )
            });
            written_rx.recv_timeout(Duration::from_secs(2)).unwrap();
            let held = merged
                .lock()
                .unwrap()
                .queue
                .iter()
                .filter(|b| !b.written)
                .count();
            resume_tx.send(()).unwrap();
            worker.join().unwrap().unwrap();
            merged
                .lock()
                .unwrap()
                .finish(Stream::Stderr, b"", &[secret.to_string()])
                .unwrap();
            assert!(
                held <= 5,
                "{secret}: {held} bytes held for a six-byte secret"
            );
        }
    }

    #[test]
    fn repeated_held_tail_still_masks_a_cross_stream_completion() {
        let values = vec!["aaaaaa".to_string(), "aaaaaX".to_string()];
        let mut m = both();
        let a = vec![b'a'; 400];
        m.arrive(Stream::Stdout, &a, &values).unwrap();
        let cut = idle_cut(&a, &values);
        m.release(Stream::Stdout, &a[..cut], &values).unwrap();
        assert!(m.queue.iter().filter(|b| !b.written).count() <= 5);
        m.arrive(Stream::Stderr, b"X", &values).unwrap();
        m.release(Stream::Stderr, b"X", &values).unwrap();
        m.finish(Stream::Stdout, &a[cut..], &values).unwrap();
        m.finish(Stream::Stderr, b"", &values).unwrap();
        assert!(!m.out.inner.contains(&b'a'));
        assert!(!m.err.inner.contains(&b'X'));
    }

    #[test]
    fn unmasked_repeated_prefix_still_masks_a_cross_stream_completion() {
        let values = vec!["aaaaab".to_string()];
        let mut m = both();
        let a = vec![b'a'; 400];
        m.arrive(Stream::Stdout, &a, &values).unwrap();
        let cut = idle_cut(&a, &values);
        m.release(Stream::Stdout, &a[..cut], &values).unwrap();
        assert!(m.queue.len() <= 5);
        m.arrive(Stream::Stderr, b"b", &values).unwrap();
        m.release(Stream::Stderr, b"b", &values).unwrap();
        m.finish(Stream::Stdout, &a[cut..], &values).unwrap();
        m.finish(Stream::Stderr, b"", &values).unwrap();
        assert!(!m.err.inner.contains(&b'b'));
    }

    #[test]
    fn distinct_secret_prefixes_share_one_idle_hold() {
        let values = vec!["aaaaab".to_string(), "bbbbbC".to_string()];
        let bytes = b"aaaaabbbbb";
        let mut m = both();
        m.arrive(Stream::Stdout, bytes, &values).unwrap();
        let cut = idle_cut(bytes, &values);
        m.release(Stream::Stdout, &bytes[..cut], &values).unwrap();
        assert_eq!(cut, 5);
        let held = m.queue.iter().filter(|b| !b.written).count();
        assert!(held <= 5, "{held} bytes held");
        m.finish(Stream::Stdout, &bytes[cut..], &values).unwrap();
        m.finish(Stream::Stderr, b"", &values).unwrap();
        assert!(!text(&m.out.inner).contains("aaaaab"));
    }

    #[test]
    fn independent_prefixes_can_complete_across_streams() {
        let values = vec!["aaaaaX".to_string(), "bbbbbC".to_string()];
        let bytes = b"aaaaabbbbb";
        let mut m = both();
        m.arrive(Stream::Stdout, bytes, &values).unwrap();
        let cut = idle_cut(bytes, &values);
        m.release(Stream::Stdout, &bytes[..cut], &values).unwrap();
        assert_eq!(cut, 5);
        let held = m.queue.iter().filter(|b| !b.written).count();
        assert!(held <= 5, "{held} bytes held");
        m.arrive(Stream::Stderr, b"X", &values).unwrap();
        m.release(Stream::Stderr, b"X", &values).unwrap();
        m.finish(Stream::Stdout, &bytes[cut..], &values).unwrap();
        m.finish(Stream::Stderr, b"", &values).unwrap();
        assert_eq!(text(&m.out.inner), "***bbbbb");
        assert_eq!(text(&m.err.inner), "***");
    }

    #[test]
    fn early_masked_prefix_stays_masked_without_completion() {
        let values = vec!["aaaaaX".to_string(), "bbbbbC".to_string()];
        let bytes = b"aaaaabbbbb";
        let mut m = both();
        m.arrive(Stream::Stdout, bytes, &values).unwrap();
        m.release(Stream::Stdout, &bytes[..5], &values).unwrap();
        assert_eq!(text(&m.out.inner), "***");
        m.finish(Stream::Stdout, &bytes[5..], &values).unwrap();
        m.finish(Stream::Stderr, b"", &values).unwrap();
        assert_eq!(text(&m.out.inner), "***bbbbb");
    }

    #[test]
    fn early_masked_prefix_still_masks_later_bytes_in_its_own_stream() {
        let values = vec!["aaaaaX".to_string(), "bbbbbC".to_string()];
        let mut m = both();
        m.arrive(Stream::Stdout, b"aaaaa", &values).unwrap();
        m.release(Stream::Stdout, b"", &values).unwrap();
        m.arrive(Stream::Stderr, b"bbbbb", &values).unwrap();
        m.release(Stream::Stderr, b"", &values).unwrap();
        assert!(m.queue.iter().filter(|b| !b.written).count() <= 5);
        m.arrive(Stream::Stderr, b"q", &values).unwrap();
        m.release(Stream::Stderr, b"bbbbbq", &values).unwrap();
        m.arrive(Stream::Stdout, b"X", &values).unwrap();
        m.release(Stream::Stdout, b"aaaaaX", &values).unwrap();
        m.finish(Stream::Stdout, b"", &values).unwrap();
        m.finish(Stream::Stderr, b"", &values).unwrap();
        assert!(!m.out.inner.contains(&b'X'));
    }

    #[test]
    fn multiple_prefixes_across_streams_share_the_hold_limit() {
        let values = ["aaaaaX", "bbbbbC", "cccccD", "dddddE"]
            .map(str::to_string)
            .to_vec();
        let mut m = both();
        for (stream, bytes) in [
            (Stream::Stdout, b"aaaaaccccc".as_slice()),
            (Stream::Stderr, b"bbbbbddddd".as_slice()),
        ] {
            m.arrive(stream, bytes, &values).unwrap();
            let cut = idle_cut(bytes, &values);
            m.release(stream, &bytes[..cut], &values).unwrap();
            let held = m.queue.iter().filter(|b| !b.written).count();
            assert!(held <= 5, "{held} bytes held");
        }
        m.finish(Stream::Stdout, b"ccccc", &values).unwrap();
        m.finish(Stream::Stderr, b"ddddd", &values).unwrap();
    }

    #[test]
    fn safe_other_stream_bytes_pass_an_early_masked_prefix() {
        let values = vec!["aaaaaX".to_string(), "bbbbbC".to_string()];
        let mut m = both();
        let bytes = b"aaaaabbbbb";
        m.arrive(Stream::Stdout, bytes, &values).unwrap();
        m.release(Stream::Stdout, &bytes[..5], &values).unwrap();
        let safe = vec![b'q'; 400];
        m.arrive(Stream::Stderr, &safe, &values).unwrap();
        m.release(Stream::Stderr, &safe, &values).unwrap();
        assert_eq!(m.err.inner, safe);
        assert!(m.queue.iter().filter(|b| !b.written).count() <= 5);
        m.finish(Stream::Stdout, &bytes[5..], &values).unwrap();
        m.finish(Stream::Stderr, b"", &values).unwrap();
    }

    #[test]
    fn periodic_patterns_keep_at_most_a_proper_prefix() {
        for secret in ["aaaaaa", "ababab", "abcabc"] {
            let values = vec![secret.to_string()];
            let bytes: Vec<u8> = secret.bytes().cycle().take(400).collect();
            let mut m = both();
            m.arrive(Stream::Stdout, &bytes, &values).unwrap();
            let cut = idle_cut(&bytes, &values);
            m.release(Stream::Stdout, &bytes[..cut], &values).unwrap();
            let held = m.queue.iter().filter(|b| !b.written).count();
            assert!(held <= 5, "{secret}: {held} bytes held");
            m.finish(Stream::Stdout, &bytes[cut..], &values).unwrap();
            m.finish(Stream::Stderr, b"", &values).unwrap();
            assert!(!text(&m.out.inner).contains(secret));
        }
    }

    /// A writer for stdout alone: stderr is already ended.
    fn stdout_only<W: Write>(out: ClosableSink<W>) -> Mutex<Merged<W, io::Sink>> {
        let mut merged = Merged::new(out, ClosableSink::new(io::sink()));
        merged.open -= 1;
        Mutex::new(merged)
    }

    /// Yields `xxxxxxxx\n` forever, one read per 20 ms, and counts its reads.
    struct Endless(Arc<AtomicUsize>);
    impl Read for Endless {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            thread::sleep(Duration::from_millis(20));
            self.0.fetch_add(1, Ordering::SeqCst);
            for (i, b) in buf.iter_mut().enumerate() {
                *b = if i % 9 == 8 { b'\n' } else { b'x' };
            }
            Ok(buf.len())
        }
    }

    struct BrokenWrite;
    impl Write for BrokenWrite {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[cfg(unix)]
    struct BrokenFlush(Vec<u8>);
    #[cfg(unix)]
    impl Write for BrokenFlush {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
    }

    #[test]
    fn pump_stops_when_the_sink_closes() {
        let reads = Arc::new(AtomicUsize::new(0));
        let endless = Endless(reads.clone());
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let merged = stdout_only(ClosableSink::new(BrokenWrite));
            let result = pump(endless, Stream::Stdout, &[], &merged);
            let merged = merged.into_inner().unwrap();
            let _ = tx.send((result, merged.out.closed));
        });
        let (result, closed) = rx
            .recv_timeout(Duration::from_secs(10))
            .expect("pump did not return after the sink closed");
        result.unwrap();
        assert!(closed, "sink did not record the close");
        // The first chunk closes the sink; the reader stops at its next read.
        thread::sleep(Duration::from_millis(200));
        let reads = reads.load(Ordering::SeqCst);
        assert!(reads <= 3, "{reads} reads after the sink closed");
    }

    #[cfg(unix)]
    #[test]
    fn pump_closes_on_flush_broken_pipe() {
        let mut child = std::process::Command::new("sh")
            .args(["-c", "printf partial; sleep 1; printf more"])
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let out = child.stdout.take().unwrap();
        let merged = stdout_only(ClosableSink::new(BrokenFlush(Vec::new())));
        pump(out, Stream::Stdout, &[], &merged).unwrap();
        let merged = merged.into_inner().unwrap();
        assert_eq!(text(&merged.out.inner.0), "partial");
        assert!(merged.out.closed);
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = tx.send(child.wait());
        });
        rx.recv_timeout(Duration::from_secs(10))
            .expect("child did not exit")
            .unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn exit_code_maps_sigpipe_to_141() {
        let code = |script: &str| {
            exit_code(
                std::process::Command::new("sh")
                    .args(["-c", script])
                    .status()
                    .unwrap(),
            )
        };
        assert_eq!(code("kill -PIPE $$"), 141);
        assert_eq!(code("exit 3"), 3);
        assert_eq!(code("kill -TERM $$"), 1);
    }

    #[test]
    fn missing_program_is_a_spawn_error_naming_it() {
        let (_tmp, _root, secrets) = sandbox();
        let err =
            run_command(&secrets, &["definitely-not-a-real-binary-xyz".to_string()]).unwrap_err();
        match err {
            SecretsError::Spawn { program, .. } => {
                assert_eq!(program, "definitely-not-a-real-binary-xyz")
            }
            other => panic!("expected Spawn, got {other:?}"),
        }
    }

    #[test]
    fn empty_argv_is_no_command() {
        let (_tmp, _root, secrets) = sandbox();
        assert!(matches!(
            run_command(&secrets, &[]).unwrap_err(),
            SecretsError::NoCommand
        ));
    }
}
