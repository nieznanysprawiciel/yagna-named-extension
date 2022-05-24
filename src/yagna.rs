use anyhow::anyhow;
use std::path::PathBuf;
use std::process::Stdio;
use sysinfo::PidExt;
use sysinfo::{Pid, ProcessExt, SystemExt};
use tokio::process::Command;

pub struct YagnaCommand {
    pub(super) cmd: Command,
}

impl YagnaCommand {
    pub fn new() -> anyhow::Result<YagnaCommand> {
        let yagna_path = parent_process()?;

        log::info!("Using yagna at: {}", yagna_path.display());

        Ok(YagnaCommand {
            cmd: Command::new(yagna_path),
        })
    }

    pub fn args(mut self, args: &Vec<String>) -> Self {
        for arg in args {
            self.cmd.arg(arg);
        }
        self
    }

    pub async fn run(self) -> anyhow::Result<serde_json::Value> {
        let mut cmd = self.cmd;
        log::debug!("Running: {:?}", cmd);
        let output = cmd
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await?;
        if output.status.success() {
            log::debug!("{}", String::from_utf8_lossy(&output.stdout));
            Ok(serde_json::from_slice(&output.stdout)
                .map_err(|e| anyhow!("Error parsing yagna command result: {}", e))?)
        } else {
            Err(anyhow::anyhow!(
                "{:?} failed.: Stdout:\n{}\nStderr:\n{}",
                cmd,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }
}

pub fn parent_process() -> anyhow::Result<PathBuf> {
    let our_pid = std::process::id();
    let sys = sysinfo::System::new_all();
    let process = sys
        .process(Pid::from_u32(our_pid))
        .ok_or(anyhow!("Can't find our own process!!"))?;

    let parent = process
        .parent()
        .map(|pid| sys.process(pid))
        .flatten()
        .ok_or(anyhow!("Can't find parent process pid."))?;
    Ok(parent.exe().to_path_buf())
}
