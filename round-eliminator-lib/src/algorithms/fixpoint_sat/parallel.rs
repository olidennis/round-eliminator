//! Optional native Gimsatul backend. Only the current size is solved; a
//! cancelled/limited call is never interpreted as exhaustion of that size.

use super::*;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

#[derive(Clone)]
pub(super) struct Settings {
    pub binary: Option<PathBuf>,
    pub total_threads: usize,
    pub initial_threads: usize,
    pub solo_threads: usize,
    min_nodes: usize,
    seconds: Option<usize>,
}

fn number(name: &str, default: usize) -> Result<usize, String> {
    match std::env::var(name) {
        Ok(s) => s
            .parse::<usize>()
            .ok()
            .filter(|n| (1..=65536).contains(n))
            .ok_or_else(|| format!("{name} must be an integer from 1 to 65536")),
        Err(std::env::VarError::NotPresent) => Ok(default),
        Err(e) => Err(format!("{name}: {e}")),
    }
}

fn executable(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    metadata.is_file()
}

fn on_path(name: &Path) -> Option<PathBuf> {
    if name.components().count() > 1 || name.is_absolute() {
        return executable(name).then(|| name.to_path_buf());
    }
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|p| p.join(name))
            .find(|p| executable(p))
    })
}

impl Settings {
    pub fn minisat() -> Self {
        Self {
            binary: None,
            total_threads: 1,
            initial_threads: 1,
            solo_threads: 1,
            min_nodes: 12,
            seconds: None,
        }
    }

    pub fn environment() -> Result<Self, String> {
        let binary = match std::env::var_os("RE_DIAGRAM_SOLVER") {
            Some(s) if s == "minisat" => None,
            Some(s) => Some(on_path(Path::new(&s)).ok_or_else(|| {
                format!(
                    "RE_DIAGRAM_SOLVER is not an executable: {}",
                    s.to_string_lossy()
                )
            })?),
            None => {
                let bundled =
                    Path::new(env!("CARGO_MANIFEST_DIR")).join("../target/native-tools/gimsatul");
                executable(&bundled)
                    .then_some(bundled)
                    .or_else(|| on_path(Path::new("gimsatul")))
            }
        };
        let total_threads = number(
            "RE_SEARCH_THREADS",
            std::thread::available_parallelism().map_or(1, usize::from),
        )?;
        let share = (total_threads / 2).max(1);
        let requested = number("RE_DIAGRAM_THREADS", share)?;
        if requested > total_threads {
            return Err("RE_DIAGRAM_THREADS must not exceed RE_SEARCH_THREADS".into());
        }
        let initial_threads = if binary.is_some() { requested } else { 1 };
        let solo_threads = if binary.is_none() {
            1
        } else if std::env::var_os("RE_DIAGRAM_THREADS").is_some() {
            requested
        } else {
            total_threads
        };
        Ok(Self {
            binary,
            total_threads,
            initial_threads,
            solo_threads,
            min_nodes: number("RE_DIAGRAM_MIN_NODES", 12)?,
            seconds: std::env::var_os("RE_DIAGRAM_SECONDS")
                .map(|_| number("RE_DIAGRAM_SECONDS", 1))
                .transpose()?,
        })
    }

    pub fn uses_external(&self, nodes: usize, options: &SatSearchOptions) -> bool {
        // Preserve the existing exact meaning of a MiniSat conflict budget.
        self.binary.is_some() && nodes >= self.min_nodes && options.conflict_limit.is_none()
    }

    pub fn certificate_threads(&self) -> usize {
        self.total_threads
            .saturating_sub(self.initial_threads)
            .max(1)
    }
}

pub(super) struct Runtime {
    pub settings: Settings,
    pub threads: AtomicUsize,
}

impl Runtime {
    pub fn new(settings: Settings, standalone: bool) -> Self {
        let threads = if standalone {
            settings.solo_threads
        } else {
            settings.initial_threads
        };
        Self {
            settings,
            threads: AtomicUsize::new(threads),
        }
    }

    pub fn release_certificate_budget(&self) {
        self.threads
            .store(self.settings.solo_threads, Ordering::Relaxed);
    }
}

// A private, exclusively created directory; no predictable shared input/output
// files, pipes that can fill up, or surviving solver process after STOP/unwind.
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Result<Self, String> {
        for _ in 0..32 {
            let path =
                std::env::temp_dir().join(format!("re-gimsatul-{:032x}", rand::random::<u128>()));
            let mut builder = fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            match builder.create(&path) {
                Ok(()) => return Ok(Self(path)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(format!("Gimsatul temporary directory: {e}")),
            }
        }
        Err("Could not create a private Gimsatul directory".into())
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Running(Child);
impl Drop for Running {
    fn drop(&mut self) {
        // Gimsatul uses threads, not child processes. Kill/wait covers them all.
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn parse_output(
    code: Option<i32>,
    reader: impl BufRead,
    variables: u32,
    clauses: &[Vec<Lit>],
    mut checkpoint: impl FnMut() -> Result<(), String>,
) -> Result<(SolverResult, Option<Assignment>), String> {
    let mut status = None;
    let mut assignment = Assignment::default();
    for line in reader.lines() {
        checkpoint()?;
        let line = line.map_err(|e| e.to_string())?;
        let line = line.trim();
        if line.starts_with("s ") {
            if status.replace(line.to_string()).is_some() {
                return Err("Gimsatul returned multiple result statuses".into());
            }
        } else if let Some(values) = line.strip_prefix("v ") {
            for value in values.split_whitespace() {
                let value: i32 = value
                    .parse()
                    .map_err(|_| "Invalid Gimsatul model literal")?;
                if value == 0 {
                    continue;
                }
                if value.unsigned_abs() > variables {
                    return Err("Out-of-range Gimsatul model literal".into());
                }
                let lit = Lit::from_ipasir(value).map_err(|e| e.to_string())?;
                if assignment.lit_value(lit) == TernaryVal::False {
                    return Err("Contradictory Gimsatul model literals".into());
                }
                assignment.assign_lit(lit);
            }
        }
    }
    match (code, status.as_deref()) {
        (Some(10), Some("s SATISFIABLE")) => {
            for clause in clauses {
                checkpoint()?;
                if !clause
                    .iter()
                    .any(|&l| assignment.lit_value(l) == TernaryVal::True)
                {
                    return Err("Gimsatul model failed independent CNF validation".into());
                }
            }
            Ok((SolverResult::Sat, Some(assignment)))
        }
        (Some(20), Some("s UNSATISFIABLE")) => Ok((SolverResult::Unsat, None)),
        (Some(0), None | Some("s UNKNOWN")) => Ok((SolverResult::Interrupted, None)),
        _ => Err(format!(
            "Gimsatul failed (exit {code:?}, status {status:?})"
        )),
    }
}

pub(super) fn solve(
    encoding: &Encoding,
    runtime: &Runtime,
    eh: &mut EventHandler,
    control: Option<&SearchControl>,
) -> Result<(SolverResult, Option<Assignment>), String> {
    let checkpoint = || -> Result<(), String> {
        search::check_event(eh)?;
        if let Some(c) = control {
            c.check()?;
        }
        Ok(())
    };
    checkpoint()?;
    let scratch = Scratch::new()?;
    let input = scratch.0.join("input.cnf");
    let output = scratch.0.join("output.txt");
    let clauses = encoding.clauses.as_ref().ok_or("Missing Gimsatul CNF")?;
    let io_error = |e: std::io::Error| format!("Gimsatul I/O: {e}");
    let mut writer = BufWriter::new(File::create(&input).map_err(io_error)?);
    writeln!(writer, "p cnf {} {}", encoding.next_var, clauses.len()).map_err(io_error)?;
    for clause in clauses {
        checkpoint()?;
        for lit in clause {
            write!(writer, "{} ", lit.to_ipasir()).map_err(io_error)?;
        }
        writeln!(writer, "0").map_err(io_error)?;
    }
    writer.flush().map_err(io_error)?;
    drop(writer);
    let threads = runtime.threads.load(Ordering::Relaxed);
    eh.notify(
        format!(
            "SAT: Gimsatul searching {}-node diagrams ({threads} threads)",
            encoding.nodes
        ),
        0,
        0,
    );
    let out = File::create(&output).map_err(io_error)?;
    let mut command = Command::new(runtime.settings.binary.as_ref().unwrap());
    command.arg("-q").arg(format!("--threads={threads}"));
    if let Some(seconds) = runtime.settings.seconds {
        command.arg(format!("--time={seconds}"));
    }
    let mut child = Running(
        command
            .arg(&input)
            .stdin(Stdio::null())
            .stdout(out.try_clone().map_err(io_error)?)
            .stderr(out)
            .spawn()
            .map_err(|e| format!("Could not start Gimsatul: {e}"))?,
    );
    let started = Instant::now();
    let mut progress = Instant::now();
    let status = loop {
        search::check_event(eh)?;
        if let Some(c) = control {
            c.check()?;
        }
        if let Some(status) = child.0.try_wait().map_err(io_error)? {
            break status;
        }
        if progress.elapsed() >= Duration::from_millis(500) {
            eh.notify(
                format!(
                    "SAT: Gimsatul searching {}-node diagrams ({threads} threads)",
                    encoding.nodes
                ),
                started.elapsed().as_secs() as usize,
                0,
            );
            progress = Instant::now();
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let file = File::open(&output).map_err(io_error)?;
    // Quiet Gimsatul writes just status/model. Bound unexpected diagnostic output.
    let cap = (u64::from(encoding.next_var) * 16 + 16384).max(2_000_000);
    if file.metadata().map_err(io_error)?.len() > cap {
        return Err("Gimsatul output exceeded its model-size bound".into());
    }
    parse_output(
        status.code(),
        BufReader::new(file.take(cap)),
        encoding.next_var,
        clauses,
        || {
            search::check_event(eh)?;
            if let Some(c) = control {
                c.check()?;
            }
            Ok(())
        },
    )
}

#[cfg(test)]
mod tests;
