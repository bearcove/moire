use std::ffi::OsStr;
use std::future::Future;
use std::io;
use std::ops::{Deref, DerefMut};
use std::process::{ExitStatus, Output, Stdio};

/// Pass-through of [`tokio::process::Command`] with API parity extras.
pub struct Command(tokio::process::Command);

#[derive(Clone, Debug)]
pub struct CommandDiagnostics {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<String>,
}

/// Pass-through of [`tokio::process::Child`] with API parity extras.
pub struct Child(tokio::process::Child);

impl Command {
    pub fn new(program: impl AsRef<OsStr>) -> Self {
        Self(tokio::process::Command::new(program))
    }

    pub fn arg(&mut self, arg: impl AsRef<OsStr>) -> &mut Self {
        self.0.arg(arg);
        self
    }

    pub fn args(&mut self, args: impl IntoIterator<Item = impl AsRef<OsStr>>) -> &mut Self {
        self.0.args(args);
        self
    }

    pub fn env(&mut self, key: impl AsRef<OsStr>, val: impl AsRef<OsStr>) -> &mut Self {
        self.0.env(key, val);
        self
    }

    pub fn envs(
        &mut self,
        vars: impl IntoIterator<Item = (impl AsRef<OsStr>, impl AsRef<OsStr>)>,
    ) -> &mut Self {
        self.0.envs(vars);
        self
    }

    pub fn env_clear(&mut self) -> &mut Self {
        self.0.env_clear();
        self
    }

    pub fn env_remove(&mut self, key: impl AsRef<OsStr>) -> &mut Self {
        self.0.env_remove(key);
        self
    }

    pub fn current_dir(&mut self, dir: impl AsRef<std::path::Path>) -> &mut Self {
        self.0.current_dir(dir);
        self
    }

    pub fn stdin(&mut self, cfg: impl Into<Stdio>) -> &mut Self {
        self.0.stdin(cfg);
        self
    }

    pub fn stdout(&mut self, cfg: impl Into<Stdio>) -> &mut Self {
        self.0.stdout(cfg);
        self
    }

    pub fn stderr(&mut self, cfg: impl Into<Stdio>) -> &mut Self {
        self.0.stderr(cfg);
        self
    }

    pub fn kill_on_drop(&mut self, kill_on_drop: bool) -> &mut Self {
        self.0.kill_on_drop(kill_on_drop);
        self
    }

    pub fn spawn(&mut self) -> io::Result<Child> {
        self.0.spawn().map(Child)
    }

    pub fn status(&mut self) -> impl Future<Output = io::Result<ExitStatus>> + '_ {
        self.0.status()
    }

    pub fn output(&mut self) -> impl Future<Output = io::Result<Output>> + '_ {
        self.0.output()
    }

    pub fn as_std(&self) -> &std::process::Command {
        self.0.as_std()
    }

    #[cfg(unix)]
    pub unsafe fn pre_exec<F>(&mut self, f: F) -> &mut Self
    where
        F: FnMut() -> io::Result<()> + Send + Sync + 'static,
    {
        self.0.pre_exec(f);
        self
    }

    pub fn into_inner(self) -> tokio::process::Command {
        self.0
    }

    pub fn into_inner_with_diagnostics(self) -> (tokio::process::Command, CommandDiagnostics) {
        let diag = CommandDiagnostics {
            program: String::new(),
            args: Vec::new(),
            env: Vec::new(),
        };
        (self.0, diag)
    }
}

impl Child {
    pub fn from_tokio_with_diagnostics(
        child: tokio::process::Child,
        _diag: CommandDiagnostics,
    ) -> Self {
        Self(child)
    }

    pub fn id(&self) -> Option<u32> {
        self.0.id()
    }

    pub fn wait(&mut self) -> impl Future<Output = io::Result<ExitStatus>> + '_ {
        self.0.wait()
    }

    pub fn wait_with_output(self) -> impl Future<Output = io::Result<Output>> {
        self.0.wait_with_output()
    }

    pub fn start_kill(&mut self) -> io::Result<()> {
        self.0.start_kill()
    }

    pub fn kill(&mut self) -> io::Result<()> {
        self.start_kill()
    }

    pub fn stdin(&mut self) -> &mut Option<tokio::process::ChildStdin> {
        &mut self.0.stdin
    }

    pub fn stdout(&mut self) -> &mut Option<tokio::process::ChildStdout> {
        &mut self.0.stdout
    }

    pub fn stderr(&mut self) -> &mut Option<tokio::process::ChildStderr> {
        &mut self.0.stderr
    }

    pub fn take_stdin(&mut self) -> Option<tokio::process::ChildStdin> {
        self.0.stdin.take()
    }

    pub fn take_stdout(&mut self) -> Option<tokio::process::ChildStdout> {
        self.0.stdout.take()
    }

    pub fn take_stderr(&mut self) -> Option<tokio::process::ChildStderr> {
        self.0.stderr.take()
    }
}

impl Deref for Child {
    type Target = tokio::process::Child;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Child {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
