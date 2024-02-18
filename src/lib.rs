use chrono::Utc;

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

/// # Panics
///
/// Will panic if any `Status::change_time` is in the future
#[allow(clippy::cast_sign_loss)]
pub fn summary_repo_age<'a, I>(it: I, ignore_archived: bool)
where
    I: IntoIterator<Item = &'a Status>,
{
    let now = Utc::now();
    //length of above + 1
    let counters: &mut [u64] = &mut [0; 9];

    'statuses: for st in it {
        if ignore_archived && st.archived.unwrap_or(false) {
            continue 'statuses;
        }
        let ch = (now - st.change_time).num_minutes();
        assert!(ch >= 0, "Time in future: {:?}!", st.change_time);
        let ix = bisection::bisect_left(BUCKETS, &(ch as u64));
        counters[ix] += 1;
    }
    for (i, j) in BUCKET_LABELS.iter().enumerate() {
        println!("{j}: {}", counters[i]);
    }
}
