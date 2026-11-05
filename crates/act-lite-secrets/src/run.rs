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

/// Result of running a command with secrets injected: exit code plus the
/// already-redacted stdout/stderr that were streamed to the caller.
#[derive(Debug)]
pub struct RunOutcome {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
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
    let merged = Mutex::new(Merged::new(
        ClosableSink::new(io::stdout()),
        ClosableSink::strict(io::stderr()),
    ));
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
        stdout: merged.stdout,
        stderr: merged.stderr,
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
    /// Arrived bytes not yet written, in the order they are written.
    queue: VecDeque<Byte>,
    /// Per stream: the last byte written was masked.
    masking: [bool; 2],
    /// Per stream: written bytes that end in an incomplete UTF-8 character.
    carry: [Vec<u8>; 2],
    /// Streams whose pump has not ended.
    open: usize,
    /// Everything written to each stream, redacted.
    stdout: String,
    stderr: String,
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
            stdout: String::new(),
            stderr: String::new(),
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

    /// Queue `bytes` just read from `stream`, mask every stored value they
    /// complete in queue order, and write what is ready.
    fn arrive(&mut self, stream: Stream, bytes: &[u8], values: &[String]) -> io::Result<()> {
        let old = self.queue.len();
        self.queue.extend(bytes.iter().map(|&byte| Byte {
            byte,
            stream,
            released: false,
            masked: false,
            starts: false,
        }));
        self.mask_queued(0, old, values);
        self.write(values)
    }

    /// Mark the next `bytes.len()` queued bytes of `stream` released, mask
    /// every stored value contiguous in them, and write what is ready.
    fn release(&mut self, stream: Stream, bytes: &[u8], values: &[String]) -> io::Result<()> {
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
        self.write(values)
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
    /// the start of the longest run of bytes before it that is a proper prefix
    /// of a stored value. Bytes after an unreleased byte can be written before
    /// it (`pass_held_tail`), so a run before it is held whatever comes next.
    /// Nothing stops once no stream is open.
    fn ready(&self, values: &[String]) -> usize {
        if self.open == 0 {
            return self.queue.len();
        }
        let u = self.unreleased();
        let vals = redactable(values);
        let longest = vals.iter().map(|v| v.len()).max().unwrap_or(0);
        (u.saturating_sub(longest)..u)
            .find(|&p| {
                vals.iter()
                    .any(|v| v.len() > u - p && self.starts_with(p, &v[..u - p]))
            })
            .unwrap_or(u)
    }

    /// Write every ready byte, in queue order, each to its own stream.
    fn write(&mut self, values: &[String]) -> io::Result<()> {
        self.pass_held_tail(values);
        let ready = self.ready(values);
        let mut piece = Vec::new();
        let mut current = None;
        for b in self.queue.drain(..ready).collect::<Vec<_>>() {
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
        }
        if let Some(stream) = current {
            self.emit(stream, &mut piece)?;
        }
        Ok(())
    }

    /// Write `bytes` to `stream` and append them to its captured text. Bytes
    /// are decoded lossily so non-UTF-8 output is tolerated; an incomplete
    /// character at the end waits for the stream's next bytes while a stream
    /// is open.
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
        let (sink, captured): (&mut dyn Write, &mut String) = match stream {
            Stream::Stdout => (&mut self.out, &mut self.stdout),
            Stream::Stderr => (&mut self.err, &mut self.stderr),
        };
        sink.write_all(text.as_bytes())?;
        sink.flush()?;
        captured.push_str(&text);
        Ok(())
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
/// `pending` that is a proper prefix of a redactable value, moved back to the
/// start of any complete value that would cross it. Bytes from this index on
/// stay pending.
fn idle_cut(pending: &[u8], values: &[String]) -> usize {
    let vals = redactable(values);
    let longest = vals.iter().map(|v| v.len()).max().unwrap_or(0);
    let mut cut = (pending.len().saturating_sub(longest)..pending.len())
        .find(|&i| {
            let tail = &pending[i..];
            vals.iter()
                .any(|v| v.len() > tail.len() && v.starts_with(tail))
        })
        .unwrap_or(pending.len());
    while let Some(i) = (cut.saturating_sub(longest)..cut).find(|&i| {
        vals.iter()
            .any(|v| i + v.len() > cut && pending[i..].starts_with(v))
    }) {
        cut = i;
    }
    cut
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
        let ready: Vec<u8> = match rx.recv() {
            Ok(Ok(chunk)) => {
                merged
                    .lock()
                    .expect("a pump thread panicked")
                    .arrive(stream, &chunk, values)?;
                pending.extend_from_slice(&chunk);
                pending.drain(..idle_cut(&pending, values)).collect()
            }
            Ok(Err(e)) => return Err(e),
            Err(mpsc::RecvError) => break,
        };
        let mut merged = merged.lock().expect("a pump thread panicked");
        merged.release(stream, &ready, values)?;
        if merged.closed(stream) {
            break;
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

    #[cfg(unix)]
    #[test]
    fn injects_secret_and_redacts_it_from_output() {
        let (_tmp, _root, mut secrets) = sandbox();
        secrets.add("MY_TOKEN", "supersecretvalue").unwrap();

        // The child echoes the secret; redaction must scrub it on the way back.
        let argv = vec![
            "sh".to_string(),
            "-c".to_string(),
            "echo $MY_TOKEN".to_string(),
        ];
        let outcome = run_command(&secrets, &argv).unwrap();
        assert_eq!(outcome.code, 0);
        assert!(outcome.stdout.contains("***"));
        assert!(!outcome.stdout.contains("supersecretvalue"));
    }

    #[cfg(unix)]
    #[test]
    fn redacts_secret_from_stderr() {
        let (_tmp, _root, mut secrets) = sandbox();
        secrets.add("MY_TOKEN", "supersecretvalue").unwrap();

        // The child echoes the secret to stderr; redaction must scrub it there too.
        let argv = vec![
            "sh".to_string(),
            "-c".to_string(),
            "echo $MY_TOKEN >&2".to_string(),
        ];
        let outcome = run_command(&secrets, &argv).unwrap();
        assert_eq!(outcome.code, 0);
        assert!(outcome.stderr.contains("***"));
        assert!(!outcome.stderr.contains("supersecretvalue"));
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
        struct Probe(Arc<Mutex<Option<Instant>>>);
        impl std::io::Write for Probe {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                let mut first = self.0.lock().unwrap();
                if first.is_none() && !buf.is_empty() {
                    *first = Some(Instant::now());
                }
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
        let probe = Probe(Arc::new(Mutex::new(None)));
        let started = Instant::now();
        let merged = stdout_only(ClosableSink::new(probe.clone()));
        pump(out, Stream::Stdout, &secrets.values(), &merged).unwrap();
        child.wait().unwrap();

        assert_eq!(merged.into_inner().unwrap().stdout, "Password: ");
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
        let argv = vec![
            "sh".to_string(),
            "-c".to_string(),
            "printf 'super'; printf 'secretvalue\\n'".to_string(),
        ];
        let outcome = run_command(&secrets, &argv).unwrap();
        assert_eq!(outcome.stdout, "***\n");
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
        assert_eq!(m.stdout, "a ");
        m.arrive(Stream::Stderr, b"secretvalue\n", &values).unwrap();
        m.release(Stream::Stderr, b"secretvalue\n", &values)
            .unwrap();
        // `super` + `secretvalue\n` can no longer start a value: stderr passes
        // stdout's held `super`.
        assert_eq!(m.stderr, "***\n");
        m.finish(Stream::Stdout, b"super", &values).unwrap();
        m.finish(Stream::Stderr, b"", &values).unwrap();
        assert_eq!(m.stdout, "a ***");
        assert_eq!(m.stderr, "***\n");
    }

    #[test]
    fn bytes_that_cannot_form_a_value_are_written_unchanged() {
        let values = vec!["supersecretvalue".to_string()];
        let mut m = both();
        m.arrive(Stream::Stdout, b"sups", &values).unwrap();
        m.release(Stream::Stdout, b"sup", &values).unwrap();
        // `sup` waits: stderr may pass the held `s` and continue it.
        assert_eq!(m.stdout, "");
        m.arrive(Stream::Stderr, b"xyz\n", &values).unwrap();
        m.release(Stream::Stderr, b"xyz\n", &values).unwrap();
        assert_eq!(m.stdout, "sup");
        assert_eq!(m.stderr, "xyz\n");
        m.finish(Stream::Stdout, b"s", &values).unwrap();
        m.finish(Stream::Stderr, b"", &values).unwrap();
        assert_eq!(m.stdout, "sups");
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
        assert_eq!(m.stdout, "***s");
        assert_eq!(m.stderr, "***\n");
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
        assert_eq!(m.stdout, "progress ");
        assert_eq!(m.stderr.len(), 128 * 8192);
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
        assert_eq!(m.stdout, "***\n");
        assert_eq!(m.stderr, "warn\n");
    }

    #[test]
    fn a_character_split_by_the_other_stream_is_written_whole() {
        let mut m = both();
        m.arrive(Stream::Stdout, b"caf\xc3", &[]).unwrap();
        m.arrive(Stream::Stderr, b"x\n", &[]).unwrap();
        m.release(Stream::Stderr, b"x\n", &[]).unwrap();
        m.arrive(Stream::Stdout, b"\xa9\n", &[]).unwrap();
        m.release(Stream::Stdout, b"caf\xc3\xa9\n", &[]).unwrap();
        assert_eq!(m.stdout, "caf\u{e9}\n");
    }

    #[test]
    fn adjacent_and_nested_values_are_masked() {
        let masked = |values: &[&str], text: &[u8]| {
            let values: Vec<String> = values.iter().map(|v| v.to_string()).collect();
            let mut m = both();
            m.arrive(Stream::Stdout, text, &values).unwrap();
            m.release(Stream::Stdout, text, &values).unwrap();
            m.stdout
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
        // A complete value that would cross the cut is held back whole.
        let values = vec!["abcdefgh".to_string(), "fghijklm".to_string()];
        assert_eq!(idle_cut(b"x abcdefghij", &values), 2);
    }

    /// A writer for stdout alone: stderr is already ended.
    fn stdout_only<W: Write>(out: ClosableSink<W>) -> Mutex<Merged<W, io::Sink>> {
        let mut merged = Merged::new(out, ClosableSink::new(io::sink()));
        merged.open -= 1;
        Mutex::new(merged)
    }

    /// Yields `xxxxxxxx\n` forever.
    struct Endless;
    impl Read for Endless {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
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

    struct BrokenFlush;
    impl Write for BrokenFlush {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Err(io::ErrorKind::BrokenPipe.into())
        }
    }

    #[test]
    fn pump_stops_when_the_sink_closes() {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let merged = stdout_only(ClosableSink::new(BrokenWrite));
            let result = pump(Endless, Stream::Stdout, &[], &merged);
            let merged = merged.into_inner().unwrap();
            let _ = tx.send((result, merged.stdout, merged.out.closed));
        });
        let (result, text, closed) = rx
            .recv_timeout(Duration::from_secs(10))
            .expect("pump did not return after the sink closed");
        result.unwrap();
        assert!(closed, "sink did not record the close");
        assert!(text.len() <= 8192, "captured {} bytes", text.len());
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
        let merged = stdout_only(ClosableSink::new(BrokenFlush));
        pump(out, Stream::Stdout, &[], &merged).unwrap();
        let merged = merged.into_inner().unwrap();
        assert_eq!(merged.stdout, "partial");
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
