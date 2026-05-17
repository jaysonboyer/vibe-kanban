//! Smoke tests for the `vibe-kanban remote` clap dispatch surface.
//!
//! These tests build and invoke the `server` binary directly (no docker
//! required). The actual `remote up` happy-path is covered by plan 07's
//! `remote_cli_e2e.rs` integration test.

use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_server")
}

#[test]
fn bin_remote_help_lists_all_subcommands() {
    let out = Command::new(bin())
        .args(["remote", "--help"])
        .output()
        .expect("server binary should run");
    assert!(out.status.success(), "exit code: {:?}", out.status.code());
    let stdout = String::from_utf8_lossy(&out.stdout);
    for needle in ["up", "down", "status", "logs", "init"] {
        assert!(
            stdout.contains(needle),
            "help should list {needle}; full output:\n{stdout}"
        );
    }
}

#[test]
fn bin_remote_up_help_lists_flags() {
    let out = Command::new(bin())
        .args(["remote", "up", "--help"])
        .output()
        .expect("server binary should run");
    assert!(out.status.success(), "exit code: {:?}", out.status.code());
    let stdout = String::from_utf8_lossy(&out.stdout);
    for needle in [
        "--with-attachments",
        "--with-relay",
        "--env-file",
        "--health-timeout",
    ] {
        assert!(
            stdout.contains(needle),
            "up --help should list {needle}; full output:\n{stdout}"
        );
    }
}

#[test]
fn bin_remote_invalid_subcommand_exits_nonzero() {
    let out = Command::new(bin())
        .args(["remote", "bogus"])
        .output()
        .expect("server binary should run");
    assert!(
        !out.status.success(),
        "bogus subcommand should fail; got status {:?}",
        out.status.code()
    );
}
