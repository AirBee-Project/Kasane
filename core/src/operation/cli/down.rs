use std::{fs, path::Path, thread, time::Duration};
use tokio::sync::watch;

use crate::PID_FILE;

pub fn down(shutdown_tx: &watch::Sender<()>) {
    if !Path::new(PID_FILE).exists() {
        println!("Server is not running.");
        return;
    }

    let pid: u32 = fs::read_to_string(PID_FILE)
        .unwrap()
        .trim()
        .parse()
        .unwrap();

    #[cfg(unix)]
    {
        use nix::sys::signal::{kill, Signal};
        use nix::unistd::Pid;
        let pid = Pid::from_raw(pid as i32);
        let _ = kill(pid, Signal::SIGTERM);
    }

    #[cfg(windows)]
    {
        let _ = shutdown_tx.send(()); // Windows では子プロセスで watch を監視
        thread::sleep(Duration::from_secs(1));
    }

    fs::remove_file(PID_FILE).unwrap_or_default();
    println!("Server stopped.");
}
