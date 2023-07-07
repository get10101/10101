use anyhow::Context;
use anyhow::Result;
use std::process::Command;
use std::process::Output;

#[cfg(target_family = "unix")] // macOS and unix
pub fn kill_process(process_name: &str) -> Result<()> {
    let output = Command::new("pkill")
        .arg(process_name)
        .output()
        .context("failed to execute 'pkill'")?;
    if !output.stderr.is_empty() {
        anyhow::bail!("Error: {}", String::from_utf8_lossy(&output.stderr));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
pub fn kill_process(process_name: &str) -> Result<()> {
    let output = Command::new("taskkill")
        .arg("/F")
        .arg("/T")
        .arg("/IM")
        .arg(process_name)
        .output()
        .context("failed to execute process")?;

    if !output.stderr.is_empty() {
        anyhow::bail!("Error: {}", String::from_utf8_lossy(&output.stderr));
    }
    Ok(())
}

pub fn is_maker_running() -> bool {
    // Prevent accidental matching on "maker" in the process list
    is_process_running("target/debug/maker")
}

fn is_process_running(process_name: &str) -> bool {
    let output: Output = Command::new("ps")
        .output()
        .expect("Failed to execute command");

    std::str::from_utf8(&output.stdout)
        .expect("to parse")
        .lines()
        .skip(1)
        .any(|line| line.contains(process_name))
}
