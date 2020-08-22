#[macro_use]
extern crate log;

use std::fs;
use std::io::prelude::Write;
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::time::Duration;

use anyhow::{Context, Result};
use globset::{Glob, GlobMatcher};
use notify::{DebouncedEvent, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use structopt::StructOpt;

const APP_NAME: &str = "Wormhole";
const RULES_FILE_NAME: &str = "rules.yaml";

#[derive(StructOpt, Debug)]
#[structopt(name = APP_NAME)]
struct Opt {
    #[structopt(name = "WATCH_DIR", required = true, parse(from_os_str))]
    watch_dir: PathBuf,

    #[structopt(short, long, default_value = "2")]
    watch_delay: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct ConfigRule {
    pattern: String,
    target: PathBuf,
}

impl ConfigRule {
    fn examples() -> [ConfigRule; 3] {
        [
            ConfigRule {
                pattern: String::from("*.jpg"),
                target: dirs::picture_dir().unwrap_or_default(),
            },
            ConfigRule {
                pattern: String::from("*.pdf"),
                target: dirs::document_dir().unwrap_or_default(),
            },
            ConfigRule {
                pattern: String::from("*.mp3"),
                target: dirs::audio_dir().unwrap_or_default(),
            },
        ]
    }
}

#[derive(Debug)]
struct Rule {
    matcher: GlobMatcher,
    target: PathBuf,
}

fn main() -> Result<()> {
    setup_logging();
    let opt = Opt::from_args();
    let config = load_or_create_config()?;
    let rules = parse_rules(&config)?;
    let (tx, rx) = channel();

    let mut watcher: RecommendedWatcher = Watcher::new(tx, Duration::from_secs(opt.watch_delay))
        .context("Could not initialize file watcher for this platform.")?;

    watcher
        .watch(&opt.watch_dir, RecursiveMode::Recursive)
        .context(format!("Could not watch directory {:#?}.", &opt.watch_dir))?;

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
                if rule.matcher.is_match(filename) {
                    if !rule_found {
                        // First rule match = highest priority match. Apply rule.
                        debug!("  Rule {} matched.", &rule.matcher.glob().to_string());
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
                            &rule.matcher.glob().to_string()
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

fn load_or_create_config() -> Result<String> {
    let config: String;

    // ensure that the config directory exists
    let config_dir = dirs::config_dir().context("Could not determine configuration directory.")?;
    let app_dir = config_dir.join(APP_NAME);
    fs::create_dir_all(&app_dir).context(format!(
        "Could not create configuration directory {:?}.",
        &app_dir
    ))?;

    // ensure that a rule file exists
    let rule_path = app_dir.join(RULES_FILE_NAME);
    if !rule_path.exists() {
        // no config file, create an example
        let mut file = fs::File::create(&rule_path).context(format!(
            "Could not create configuration file {:?}.",
            &rule_path
        ))?;
        config = String::from(serde_yaml::to_string(&ConfigRule::examples()).unwrap());
        file.write_all(config.as_bytes()).unwrap();
        info!("Created example configuration {:?}.", &rule_path);
    } else {
        // use existing config
        config = fs::read_to_string(&rule_path).context(format!(
            "Could not read configuration file {:#?}.",
            &rule_path
        ))?;
        info!("Found existing configuration {:?}.", &rule_path);
    }

    Ok(config)
}

fn parse_rules(config: &str) -> Result<Vec<Rule>> {
    let yaml: Vec<ConfigRule> =
        serde_yaml::from_str(config).context("Failed to parse rule configuration.")?;

    let rules: Vec<Rule> = yaml
        .into_iter()
        .filter_map(|r| match Glob::new(&r.pattern) {
            Ok(glob) => {
                // check for problems with this target
                if r.target.is_relative() {
                    error!(
                        "Target {:?} is not an absolute path. Rule ignored.",
                        &r.target
                    );
                    None
                } else if !r.target.exists() {
                    error!("Target {:?} does not exist. Rule ignored.", &r.target);
                    None
                } else if !r.target.is_dir() {
                    error!("Target {:?} is not a directory. Rule ignored.", &r.target);
                    None
                } else {
                    // valid target
                    Some(Rule {
                        matcher: glob.compile_matcher(),
                        target: r.target.clone(),
                    })
                }
            }
            Err(e) => {
                error!(
                    "Pattern {} cannot be compiled. Rule ignored. Reason: {}.",
                    &r.pattern, e
                );
                None
            }
        })
        .collect();

    info!("Successfully parsed {} rules.", rules.len());
    Ok(rules)
}
