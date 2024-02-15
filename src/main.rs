use std::{
    collections::HashMap,
    error::Error,
    fmt::Display,
    fs::{create_dir_all, read_to_string},
    io::ErrorKind,
    path::PathBuf,
    process::ExitCode,
    str::FromStr,
};

use chrono::{DateTime, FixedOffset, TimeZone, Utc};
use clap::{Parser, Subcommand};
use rand::{rngs::StdRng, Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Hash {
    Sha1([u8; 20]),
    Sha256([u8; 32]),
}

impl Hash {
    fn sha1_from_bytes(s: &[u8]) -> Result<Self, String> {
        Ok(Self::Sha1(s.try_into().map_err(|e| {
            format!("Hash length should be 32 bits: {e}")
        })?))
    }
    fn sha256_from_bytes(s: &[u8]) -> Result<Self, String> {
        Ok(Self::Sha256(s.try_into().map_err(|e| {
            format!("Hash length should be 32 bits: {e}")
        })?))
    }
}

impl FromStr for Hash {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let l = s.len();
        let d = hex::decode(s).map_err(|e| format!("Could not decode hash from hex '{s}': {e}"))?;
        if l == 40 {
            Self::sha1_from_bytes(&d)
        } else if l == 64 {
            Self::sha256_from_bytes(&d)
        } else {
            Err(format!(
                "Unexpected length of hash '{s}' ({l} chars), expecting 40/64 hex chars"
            ))
        }
    }
}

impl AsRef<[u8]> for Hash {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::Sha1(s) => &s[..],
            Self::Sha256(s) => &s[..],
        }
    }
}

impl Display for Hash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", hex::encode(self))
    }
}

#[serde_as]
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
struct Status {
    check_time: DateTime<Utc>,
    change_time: DateTime<Utc>,
    #[serde_as(as = "DisplayFromStr")]
    commit_hash: Hash,
}

#[derive(Parser, Debug, Clone)]
#[command(version, about, long_about = None)]
struct Options {
    #[arg(short, long)]
    data_file: PathBuf,
    path: PathBuf,

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
    fn get_config(&self) -> Result<Option<HashMap<String, Status>>, Box<dyn Error>> {
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
    fn write(&self, config: &HashMap<String, Status>) -> Result<(), Box<dyn Error>> {
        let p = &self.data_file;
        if let Some(d) = p.parent() {
            create_dir_all(d)?;
        }
        std::fs::write(p, toml::to_string(config)?)?;
        Ok(())
    }
    fn get_config_key(&self) -> &str {
        self.path.to_str().expect("Paths should be unicode")
    }
}

#[allow(clippy::cast_precision_loss)]
fn calculate_probability<Tz: TimeZone>(
    last_change: DateTime<Tz>,
    last_check: DateTime<Tz>,
    now: DateTime<Tz>,
) -> f64 {
    let days = (last_check.clone() - last_change).num_days();
    log::debug!("days with no update: {days}");
    if days <= 0 {
        return 1.0;
    }
    let prob = 3.0 / days as f64;
    log::debug!("probability then of change: {prob}");
    let elapsed = (now - last_check).num_days();
    log::debug!("days elapsed: {elapsed}");
    if elapsed <= 0 {
        return prob;
    }
    prob * elapsed as f64
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
        let hash = blake3::hash(s.as_ref());
        StdRng::from_seed(*hash.as_bytes())
    } else {
        StdRng::from_rng(rand::thread_rng()).expect("Should create StdRng")
    }
}

fn do_check<T: AsRef<[u8]>>(seed: Option<T>, status: Option<Status>) -> ExitCode {
    if let Some(st) = status {
        let mut rng = get_rng(seed);
        if should_run_now(&mut rng, st.change_time, st.check_time) {
            ExitCode::FAILURE
        } else {
            ExitCode::SUCCESS
        }
    } else {
        ExitCode::FAILURE
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
                .unwrap_or_else(HashMap::default);
            conf.insert(
                args.get_config_key().to_owned(),
                Status {
                    commit_hash: commit_hash.to_owned(),
                    change_time: commit_time.to_utc(),
                    check_time: Utc::now(),
                },
            );
            args.write(&conf).expect("Should write config to file");
            ExitCode::SUCCESS
        }
    }
}
