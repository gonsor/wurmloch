use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::time::Duration;

use anyhow::{Context, Result};
use app_dirs2::{get_app_root, AppDataType, AppInfo};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use structopt::StructOpt;

const RULES_FILE_NAME: &str = "rules.yaml";

#[derive(StructOpt, Debug)]
#[structopt(name = "Wormhole")]
struct Opt {
    #[structopt(short, long, required = true, parse(from_os_str))]
    watch_dir: PathBuf,

    #[structopt(short = "e", long, default_value = "2")]
    watch_delay: u64,
}

fn main() -> Result<()> {
    let opt = Opt::from_args();

    setup_path()?;

    let (tx, rx) = channel();

    let mut watcher: RecommendedWatcher = Watcher::new(tx, Duration::from_secs(opt.watch_delay))
        .context("Could not initialize file watcher for this platform.")?;

    watcher
        .watch(&opt.watch_dir, RecursiveMode::Recursive)
        .context(format!(
            "Could not watch directory '{:#?}'.",
            &opt.watch_dir
        ))?;

    loop {
        match rx.recv() {
            Ok(event) => println!("XD"),
            Err(e) => eprintln!("{}", e),
        }
    }

    Ok(())
}

fn setup_path() -> Result<()> {
    let dir = get_app_root(
        AppDataType::UserConfig,
        &AppInfo {
            name: "Wormhole",
            author: "May",
        },
    )?;
    fs::create_dir_all(&dir).context(format!(
        "Could not create configuration directory '{:#?}'.",
        &dir
    ))?;

    //let rules = dir.join(RULES_FILE_NAME);

    Ok(())
}
