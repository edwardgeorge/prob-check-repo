use chrono::{DateTime, Utc};

mod data;
pub use data::{Hash, Status};

// num of minutes in each, roughly...
const HOURS: u64 = 60;
const DAYS: u64 = HOURS * 24;
const WEEKS: u64 = DAYS * 7;
const MONTHS: u64 = DAYS * 30;
const YEARS: u64 = DAYS * 365;
static BUCKETS: &[u64] = &[
    (24 * HOURS),
    (3 * DAYS),
    WEEKS,
    (3 * WEEKS),
    (3 * MONTHS),
    YEARS,
    (3 * YEARS),
    (10 * YEARS),
];
static BUCKET_LABELS: &[&str] = &[
    "< 1 Day",
    "< 3 Days",
    "< 1 Week",
    "< 3 Weeks",
    "< 3 Months",
    "< 1 Year",
    "< 3 Years",
    "< 10 Years",
    "10 Years +",
];

pub fn summary_repo_age<'a, I>(it: I, ignore_archived: bool)
where
    I: IntoIterator<Item = &'a Status>,
{
    summarise_age_by(it, |st| {
        if ignore_archived && st.archived.unwrap_or(false) {
            None
        } else {
            Some(st.change_time)
        }
    });
}

pub fn summary_check_age<'a, I>(it: I)
where
    I: IntoIterator<Item = &'a Status>,
{
    summarise_age_by(it, |st| Some(st.check_time));
}

#[allow(clippy::cast_sign_loss)]
fn summarise_age_by<'a, I, F>(it: I, by: F)
where
    I: IntoIterator<Item = &'a Status>,
    F: Fn(&'a Status) -> Option<DateTime<Utc>>,
{
    let now = Utc::now();
    let counters: &mut [u64] = &mut [0; 9];
    for st in it {
        if let Some(t) = by(st) {
            let ch = (now - t).num_minutes();
            assert!(ch >= 0, "Time in future: {:?}!", st.change_time);
            let ix = bisection::bisect_left(BUCKETS, &(ch as u64));
            counters[ix] += 1;
        }
    }
    for (i, j) in BUCKET_LABELS.iter().enumerate() {
        println!("{j}: {}", counters[i]);
    }
}
