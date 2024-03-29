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

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Clone, Subcommand)]
enum Command {
    Check {
        #[arg(short = 'n', long = "name")]
        name: String,
        #[arg(short, long)]
        seed: Option<String>,
    },
    Record {
        #[arg(short = 'n', long = "name")]
        name: String,
        #[arg(short = 't', long)]
        commit_time: DateTime<FixedOffset>,
        #[arg(short = 'c', long)]
        commit_hash: Hash,
    },
    Summarise {
        #[command(subcommand)]
        ty: Summary,
    },
}

#[derive(Debug, Clone, Copy, Subcommand)]
enum Summary {
    RepoAge,
    CheckTime,
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
    fn write(&self, config: &Map) -> Result<(), Box<dyn Error>> {
        let p = self.data_file.canonicalize()?;
        if let Some(d) = p.parent() {
            create_dir_all(d)?;
        }
        log::debug!("Writing config to {}", p.display());
        std::fs::write(p, toml::to_string(config)?)?;
        Ok(())
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
    let x = mins1 as f64 / mins2 as f64;
    3.0 / x
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
        if st.archived.unwrap_or(false) {
            return ExitCode::FAILURE;
        }
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
        Command::Check { ref seed, ref name } => {
            return do_check(
                seed.as_ref(),
                args.get_config()
                    .expect("Should config")
                    .and_then(|mut m| m.remove(name)),
            );
        }
        Command::Record {
            ref name,
            ref commit_hash,
            ref commit_time,
        } => {
            let mut conf = args
                .get_config()
                .expect("Should read config")
                .unwrap_or_else(Map::default);
            log::debug!("Updating status for {name}");
            conf.entry(name.to_owned())
                .and_modify(|s| {
                    s.commit_hash = commit_hash.clone();
                    s.change_time = commit_time.to_utc();
                    s.check_time = Utc::now();
                })
                .or_insert_with(|| Status {
                    commit_hash: commit_hash.to_owned(),
                    change_time: commit_time.to_utc(),
                    check_time: Utc::now(),
                    archived: None,
                });
            args.write(&conf).expect("Should write config to file");
            ExitCode::SUCCESS
        }
        Command::Summarise { ty } => {
            let v = args.get_config().unwrap().expect("Data file not found");
            match ty {
                Summary::RepoAge => prob_check_repo::summary_repo_age(v.values(), true),
                Summary::CheckTime => prob_check_repo::summary_check_age(v.values()),
            }
            ExitCode::SUCCESS
        }
    }
}
