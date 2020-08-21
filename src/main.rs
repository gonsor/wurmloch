#[macro_use]
extern crate log;

use std::fs;
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::time::Duration;

use anyhow::{Context, Result};
use app_dirs2::{get_app_root, AppDataType, AppInfo};
use globset::{Glob, GlobMatcher};
use notify::{DebouncedEvent, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Deserialize;
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

#[derive(Debug, Deserialize)]
struct YAMLRule {
    pattern: String,
    target: PathBuf,
}

#[derive(Debug)]
struct Rule {
    pattern: GlobMatcher,
    target: PathBuf,
}

impl From<YAMLRule> for Rule {
    fn from(yaml: YAMLRule) -> Self {
        Self {
            pattern: Glob::new(&yaml.pattern).unwrap().compile_matcher(),
            target: yaml.target.clone(),
        }
    }
}

fn main() -> Result<()> {
    setup_logging();
    let opt = Opt::from_args();
    let rules = parse_rules()?;
    let (tx, rx) = channel();

    let mut watcher: RecommendedWatcher = Watcher::new(tx, Duration::from_secs(opt.watch_delay))
        .context("Could not initialize file watcher for this platform.")?;

    watcher
        .watch(&opt.watch_dir, RecursiveMode::Recursive)
        .context(format!(
            "Could not watch directory {:#?}.",
            &opt.watch_dir
        ))?;

    info!("Watching {:?} ...", &opt.watch_dir);

    loop {
        match rx.recv() {
            Ok(event) => handle_event(&rules, &event)?,
            Err(e) => error!("{}", e),
        }
    }
}

fn setup_logging() {
    // change default log level
    // TODO: use custom environment variable
    //if std::env::var("RUST_LOG").is_err() {
    //    std::env::set_var("RUST_LOG", "info");
    //}
    pretty_env_logger::init();
}

fn handle_event(rules: &[Rule], event: &DebouncedEvent) -> Result<()> {
    if let DebouncedEvent::Create(path) = event {
        if let Some(filename) = path.file_name() {
            debug!("Processing {:?}.", filename);
            let mut rule_found = false;
            for rule in rules.iter() {
                if rule.pattern.is_match(filename) {
                    if !rule_found {
                        // First rule match = highest priority match. Apply rule.
                        debug!("  Rule {} matched.", &rule.pattern.glob().to_string());
                        match fs::rename(&path, &rule.target.join(filename)) {
                            Ok(_) => {
                                debug!("    Moved {:?} to {:?}.", filename, &rule.target);
                                rule_found = true;
                            }
                            Err(e) => {
                                error!("Could not move {:?} to {:?}.", filename, &rule.target);
                                error!("Reason: {}.", e);
                            }
                        }
                    } else {
                        // Consecutive rule matches are ignored
                        debug!(
                            "  Rule '{}' would have also matched but has lower priority.",
                            &rule.pattern.glob().to_string()
                        );
                    }
                }
            }
            if !rule_found {
                warn!("No rule found for file {:?}. Ignored.", filename);
            }
        }
    }
    Ok(())
}

fn parse_rules() -> Result<Vec<Rule>> {
    let dir = get_app_root(
        AppDataType::UserConfig,
        &AppInfo {
            name: "Wormhole",
            author: "May",
        },
    )?;
    fs::create_dir_all(&dir).context(format!(
        "Could not create configuration directory {:#?}.",
        &dir
    ))?;
    let rules_path = dir.join(RULES_FILE_NAME);

    let contents = fs::read_to_string(&rules_path).context(format!(
        "Failed to read rule configuration file at {:#?}.",
        &rules_path
    ))?;

    let rules: Vec<YAMLRule> =
        serde_yaml::from_str(&contents).context("Failed to parse rule configuration.")?;
    let rules: Vec<Rule> = rules.into_iter().map(|y| y.into()).collect();

    info!("Successfully parsed {} rules.", rules.len());

    Ok(rules)
}
