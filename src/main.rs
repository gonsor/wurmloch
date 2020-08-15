use anyhow::{Context, Result};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::sync::mpsc::channel;
use std::time::Duration;

const WATCH_PATH: &str = ".wormhole";
const WATCH_DELAY: u64 = 3;

fn main() -> Result<()> {
    let (tx, rx) = channel();

    let mut watcher: RecommendedWatcher = Watcher::new(tx, Duration::from_secs(WATCH_DELAY))
        .context("Could not initialize file watcher for this platform.")?;

    watcher
        .watch(WATCH_PATH, RecursiveMode::Recursive)
        .context(format!("Could not watch directory '{}'.", WATCH_PATH))?;

    loop {
        match rx.recv() {
            Ok(event) => println!("XD"),
            Err(e) => eprintln!("{}", e),
        }
    }

    Ok(())
}
