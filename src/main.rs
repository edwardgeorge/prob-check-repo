use std::{
    collections::BTreeMap,
    error::Error,
    fs::{create_dir_all, read_to_string},
    io::ErrorKind,
    path::PathBuf,
    process::ExitCode,
};

use chrono::{DateTime, FixedOffset, TimeZone, Utc};
use clap::{Parser, Subcommand};
use rand::{rngs::StdRng, Rng, SeedableRng};

use prob_check_repo::{Hash, Status};

type Map = BTreeMap<String, Status>;

#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
struct Options {
    #[arg(short, long)]
    data_file: PathBuf,
    #[arg(short = 'n', long = "name")]
    path: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Clone, Subcommand)]
enum Command {
    Check {
        #[arg(short, long)]
        seed: Option<String>,
    },
    Record {
        #[arg(short = 't', long)]
        commit_time: DateTime<FixedOffset>,
        #[arg(short = 'c', long)]
        commit_hash: Hash,
    },
}

impl Options {
    fn get_config(&self) -> Result<Option<Map>, Box<dyn Error>> {
        let s = match read_to_string(&self.data_file) {
            Ok(s) => s,
            Err(e) => {
                return if e.kind() == ErrorKind::NotFound {
                    Ok(None)
                } else {
                    Err(e)?
                };
            }
        };
        Ok(toml::from_str(&s)?)
    }
    fn get_status(&self) -> Result<Option<Status>, Box<dyn Error>> {
        Ok(self
            .get_config()?
            .and_then(|mut m| m.remove(self.get_config_key())))
    }
    fn write(&self, config: &Map) -> Result<(), Box<dyn Error>> {
        let p = self.data_file.canonicalize()?;
        if let Some(d) = p.parent() {
            create_dir_all(d)?;
        }
        log::debug!("Writing config to {}", p.display());
        std::fs::write(p, toml::to_string(config)?)?;
        Ok(())
    }
    fn get_config_key(&self) -> &str {
        &self.path
    }
}

#[allow(clippy::cast_precision_loss)]
fn calculate_probability<Tz: TimeZone>(
    last_change: DateTime<Tz>,
    last_check: DateTime<Tz>,
    now: DateTime<Tz>,
) -> f64 {
    let mins1 = (last_check.clone() - last_change).num_minutes();
    let mins2 = (now - last_check).num_minutes();
    if mins1 <= 0 || mins2 <= 0 || mins2 > mins1 {
        return 1.0;
    }
    let x = mins1 / mins2;
    // should never be divbyzero due to mins2 > mins1 check above
    3.0 / x as f64
}

fn should_run_now<Tz: TimeZone, R: Rng>(
    rng: &mut R,
    last_change: DateTime<Tz>,
    last_check: DateTime<Tz>,
) -> bool {
    let now = Utc::now().with_timezone(&last_change.timezone());
    let prob = calculate_probability(last_change, last_check, now);
    let v = rng.gen::<f64>();
    log::debug!("target probability: {prob}, rand value: {v}");
    v <= prob
}

fn get_rng<T: AsRef<[u8]>>(seed: Option<T>) -> StdRng {
    if let Some(s) = seed {
        let i = s.as_ref();
        if !i.is_empty() {
            let hash = blake3::hash(i);
            return StdRng::from_seed(*hash.as_bytes());
        }
    }
    StdRng::from_rng(rand::thread_rng()).expect("Should create StdRng")
}

fn do_check<T: AsRef<[u8]>>(seed: Option<T>, status: Option<Status>) -> ExitCode {
    if let Some(st) = status {
        let mut rng = get_rng(seed);
        if should_run_now(&mut rng, st.change_time, st.check_time) {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        }
    } else {
        ExitCode::SUCCESS
    }
}

fn main() -> ExitCode {
    env_logger::init();
    let args = Options::parse();
    match args.command {
        Command::Check { ref seed } => {
            return do_check(
                seed.as_ref(),
                args.get_status().expect("Should read status"),
            );
        }
        Command::Record {
            ref commit_hash,
            ref commit_time,
        } => {
            let mut conf = args
                .get_config()
                .expect("Should read config")
                .unwrap_or_else(Map::default);
            let key = args.get_config_key();
            log::debug!("Updating status for {key}");
            let x = conf.insert(
                args.get_config_key().to_owned(),
                Status {
                    commit_hash: commit_hash.to_owned(),
                    change_time: commit_time.to_utc(),
                    check_time: Utc::now(),
                },
            );
            if log::log_enabled!(log::Level::Debug) {
                log::debug!(
                    "{} '{key}'",
                    if x.is_none() { "Created" } else { "Updated" }
                );
            }
            args.write(&conf).expect("Should write config to file");
            ExitCode::SUCCESS
        }
    }
}
