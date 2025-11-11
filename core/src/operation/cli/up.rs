use crate::PID_FILE;
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn up() {
    if Path::new(PID_FILE).exists() {
        println!("Kasane is already running.");
        return;
    }

    let exe = std::env::current_exe().expect("Failed to get current exe");

    let child = Command::new(exe)
        .spawn()
        .expect("Failed to start kasane server");

    fs::write(PID_FILE, child.id().to_string()).expect("Failed to write PID file");

    println!("Kasane server started with PID {}", child.id());
}
