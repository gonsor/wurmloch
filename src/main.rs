#[macro_use]
extern crate log;

#[macro_use]
extern crate anyhow;

use std::fs;
use std::io::prelude::Write;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender};
use std::time::Duration;

use anyhow::{Context, Result};
use clap::Parser;
use globset::{Glob, GlobMatcher};
use notify::{DebouncedEvent, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};

const APP_NAME: &str = "Wurmloch";
const RULES_FILE_NAME: &str = "rules.yaml";

/// Sort your filesystem by turning a folder into a wormhole
#[derive(Parser, Debug)]
#[clap(name = APP_NAME)]
struct Args {
    /// This directory will be turned into a wormhole
    #[clap(name = "WATCH_DIR", required = true, parse(from_os_str))]
    watch_dir: PathBuf,

    /// React to file events after this delay (in seconds)
    #[clap(short, long, default_value = "2")]
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
    pretty_env_logger::init_custom_env(&format!("{}_LOG", APP_NAME.to_uppercase()));
    let args = Args::parse();

    check_watch_directory(&args.watch_dir)?;

    let (config_path, config) = load_or_create_config()?;
    let mut rules = parse_rules(&config)?;
    let (tx, rx) = channel();

    // Start watching
    let watch_delay = Duration::from_secs(args.watch_delay);
    let _conf_watcher = watch(Sender::clone(&tx), &config_path, watch_delay);
    let _dir_watcher = watch(tx, &args.watch_dir, watch_delay);

    loop {
        match rx.recv() {
            Ok(event) => match event {
                DebouncedEvent::Create(path) => handle_file(&rules, &path)?,
                DebouncedEvent::Write(path) => {
                    if path == config_path {
                        // Configuration file changed
                        rules = parse_rules(&fs::read_to_string(&path).unwrap())?;
                    }
                }
                _ => trace!("Unhandled notify event: {:#?}.", event),
            },
            Err(e) => error!("{}", e),
        }
    }
}

fn check_watch_directory(path: &Path) -> Result<()> {
    if path.is_relative() {
        return Err(anyhow!(
            "Watch directory {:?} must be an absolute path.",
            path
        ));
    } else if !path.exists() {
        return Err(anyhow!("Watch directory {:?} does not exist.", path));
    } else if !path.is_dir() {
        return Err(anyhow!("Watch directory {:?} is not a directory.", path));
    }
    Ok(())
}

fn watch(
    tx: Sender<DebouncedEvent>,
    path: &Path,
    watch_delay: Duration,
) -> Result<RecommendedWatcher> {
    let mut watcher: RecommendedWatcher = Watcher::new(tx, watch_delay)
        .context("Could not initialize file watcher for this platform.")?;

    watcher
        .watch(path, RecursiveMode::Recursive)
        .context(format!("Could not watch {:#?}.", path))?;

    info!("Watching {:?} ...", path);
    Ok(watcher)
}

fn handle_file(rules: &[Rule], path: &Path) -> Result<()> {
    if let Some(filename) = path.file_name() {
        debug!(" --- Processing {:?} --- ", filename);
        let mut rule_found = false;
        for rule in rules.iter() {
            if rule.matcher.is_match(filename) {
                if !rule_found {
                    // First rule match = highest priority match. Apply rule.
                    debug!("Rule {} matched.", &rule.matcher.glob().to_string());
                    match fs::rename(&path, &rule.target.join(filename)) {
                        Ok(_) => {
                            debug!("Moved {:?} to {:?}.", filename, &rule.target);
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
                        "Rule '{}' would have also matched but has lower priority.",
                        &rule.matcher.glob().to_string()
                    );
                }
            }
        }
        if !rule_found {
            warn!("No rule found for file {:?}. Ignored.", filename);
        }
    }
    Ok(())
}

fn load_or_create_config() -> Result<(PathBuf, String)> {
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

    Ok((rule_path, config))
}

fn is_valid_target(path: &Path) -> bool {
    if path.is_relative() {
        error!("Target {:?} is not an absolute path. Rule ignored.", &path);
        return false;
    } else if !path.exists() {
        error!("Target {:?} does not exist. Rule ignored.", &path);
        return false;
    } else if !path.is_dir() {
        error!("Target {:?} is not a directory. Rule ignored.", &path);
        return false;
    }
    true
}

fn parse_rules(config: &str) -> Result<Vec<Rule>> {
    info!("Parsing rules ...");

    let yaml: Vec<ConfigRule> =
        serde_yaml::from_str(config).context("Failed to parse rule configuration.")?;

    let rules: Vec<Rule> = yaml
        .into_iter()
        .filter_map(|r| match Glob::new(&r.pattern) {
            Ok(glob) => {
                if is_valid_target(&r.target) {
                    Some(Rule {
                        matcher: glob.compile_matcher(),
                        target: r.target,
                    })
                } else {
                    None
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
