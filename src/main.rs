use std::{
    env,
    error::Error,
    fmt::Display,
    fs::{self, File},
    io::{self, ErrorKind, Read, Write},
    marker::PhantomData,
    path::Path,
};

use anyhow::{Result, anyhow, ensure};
use chrono::{DateTime, Days, NaiveDate, NaiveDateTime, NaiveTime, TimeDelta, Timelike, Utc};
use decent::{Decodable, Encodable, PrimitiveRepr, Version};
use decent_macros::Binary;

pub const LATEST: Version = Version(0, 0, 2);

pub fn encode_datetime(
    date: &DateTime<Utc>,
    to: &mut dyn Write,
    version: Version,
    repr: PrimitiveRepr,
) -> io::Result<()> {
    let naive = date.naive_utc();
    let date = naive.date();
    let time = naive.time();
    date.to_epoch_days().encode(to, version, repr)?;
    time.num_seconds_from_midnight().encode(to, version, repr)?;
    time.nanosecond().encode(to, version, repr)?;
    Ok(())
}
pub fn decode_datetime(
    from: &mut dyn Read,
    version: Version,
    repr: PrimitiveRepr,
) -> io::Result<DateTime<Utc>> {
    let epoch_day_count = i32::decode(from, version, repr)?;
    let second_count = u32::decode(from, version, repr)?;
    let nanosecond_count = u32::decode(from, version, repr)?;
    let date = NaiveDate::from_epoch_days(epoch_day_count).ok_or_else(|| {
        io::Error::new(
            ErrorKind::InvalidData,
            "invalid date while decoding date and time",
        )
    })?;
    let time = NaiveTime::from_num_seconds_from_midnight_opt(second_count, nanosecond_count)
        .ok_or_else(|| {
            io::Error::new(
                ErrorKind::InvalidData,
                "invalid time while decoding date and time",
            )
        })?;
    let naive = NaiveDateTime::new(date, time);
    Ok(DateTime::from_naive_utc_and_offset(naive, Utc))
}

pub fn encode_timedelta(
    delta: &TimeDelta,
    to: &mut dyn Write,
    version: Version,
    repr: PrimitiveRepr,
) -> io::Result<()> {
    delta
        .num_nanoseconds()
        .ok_or_else(|| {
            io::Error::new(ErrorKind::InvalidData, "number of nanoseconds is too large")
        })?
        .encode(to, version, repr)
}
pub fn decode_timedelta(
    from: &mut dyn Read,
    version: Version,
    repr: PrimitiveRepr,
) -> io::Result<TimeDelta> {
    Ok(TimeDelta::nanoseconds(i64::decode(from, version, repr)?))
}

pub fn today() -> DateTime<Utc> {
    DateTime::from_naive_utc_and_offset(
        NaiveDateTime::new(
            Utc::now().date_naive(),
            NaiveTime::from_num_seconds_from_midnight_opt(0, 0).unwrap(),
        ),
        Utc,
    )
}

pub fn try_split_once<'a>(all: &'a str, delimiter: &str) -> (&'a str, Option<&'a str>) {
    match all.split_once(delimiter) {
        Some((first, rest)) => (first, Some(rest)),
        None => (all, None),
    }
}

// #[derive(Clone, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
// pub struct StringError(pub String);
// pub fn error_str<T>(str: String) -> Result<T> {
//     return Err(anyhow::Error::new(StringError(str)));
// }
// impl Display for StringError {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         write!(f, "{}", self.0)
//     }
// }
// impl Error for StringError {}

pub fn parse_timedelta(hhmmss: &str) -> Result<TimeDelta> {
    let mut delta = TimeDelta::zero();
    let (hh, maybe_mmss) = try_split_once(hhmmss, ":");
    ensure!(hh.len() == 2, "expected 2 hour digits, got {}", hh.len());
    match hh.parse::<u8>()? {
        hours @ 0..=23 => {
            delta += TimeDelta::hours(hours as i64);
        }
        oob => return Err(anyhow!("`{oob}` out of bounds (expected 0 to 23 hours)")),
    }

    let Some(mmss) = maybe_mmss else {
        return Ok(delta);
    };
    let (mm, maybe_ss) = try_split_once(mmss, ":");
    ensure!(mm.len() == 2, "expected 2 minute digits, got {}", mm.len());
    match mm.parse::<u8>()? {
        minutes @ 0..=59 => {
            delta += TimeDelta::minutes(minutes as i64);
        }
        oob => return Err(anyhow!("`{oob}` out of bounds (expected 0 to 59 minutes)")),
    }

    let Some(ss) = maybe_ss else {
        return Ok(delta);
    };
    ensure!(ss.len() == 2, "expected 2 minute digits, got {}", ss.len());
    match ss.parse::<u8>()? {
        seconds @ 0..=59 => {
            delta += TimeDelta::seconds(seconds as i64);
        }
        oob => return Err(anyhow!("`{oob}` out of bounds (expected 0 to 59 seconds)")),
    }

    return Ok(delta);
}

pub fn parse_date(repr: &str) -> Result<DateTime<Utc>> {
    let date = match repr {
        "now" => Utc::now(),
        "today" => today(),
        "tomorrow" | "tmrw" => today() + Days::new(1),
        unknown => {
            return Err(anyhow!("`{unknown}` is not a valid date"));
        }
    };
    return Ok(date);
}

pub fn parse_datetime(repr: &str) -> Result<DateTime<Utc>> {
    let (date_repr, maybe_hhmmss) = try_split_once(repr, "+");
    let mut date = parse_date(date_repr)?;
    let Some(hhmmss) = maybe_hhmmss else {
        return Ok(date);
    };
    date += parse_timedelta(hhmmss)?;
    Ok(date)
}

pub trait ScheduleID {
    const BYTES: [u8; 8];
    const NAME: &'static str;
}
#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct ID<T: ScheduleID>(pub PhantomData<T>);
impl<T: ScheduleID> Encodable for ID<T> {
    fn encode(&self, to: &mut dyn Write, _: Version, _: PrimitiveRepr) -> io::Result<()> {
        to.write_all(&T::BYTES)
    }
}
impl<T: ScheduleID> Decodable for ID<T> {
    fn decode(from: &mut dyn Read, _: Version, _: PrimitiveRepr) -> io::Result<Self> {
        let mut bytes = vec![0u8; T::BYTES.len()];
        from.read_exact(&mut bytes)?;
        if bytes != T::BYTES {
            return Err(io::Error::new(
                ErrorKind::InvalidData,
                format!("incorrect identifier for {}", T::NAME),
            ));
        }
        return Ok(Self(PhantomData));
    }
}

#[derive(Binary, Clone, Copy, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
struct RegularSchedule {
    #[version]
    version: Version,
    #[since(0, 0, 2)]
    id: ID<Self>,
    #[encode_with(encode_datetime)]
    #[decode_with(decode_datetime)]
    next: DateTime<Utc>,
    #[encode_with(encode_timedelta)]
    #[decode_with(decode_timedelta)]
    interval: TimeDelta,
}
impl RegularSchedule {
    pub fn create(start: DateTime<Utc>, every: TimeDelta) -> Self {
        Self {
            version: LATEST,
            id: ID(PhantomData),
            next: start,
            interval: every,
        }
    }
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self::decode(
            &mut File::open(&path)?,
            Version::ZERO,
            PrimitiveRepr::Varint,
        )?)
    }
    pub fn save(&mut self, path: impl AsRef<Path>) -> Result<()> {
        self.version = LATEST;
        Ok(self.encode(&mut File::create(&path)?, LATEST, PrimitiveRepr::Varint)?)
    }
}
impl ScheduleID for RegularSchedule {
    const BYTES: [u8; 8] = *b"regular ";
    const NAME: &'static str = "regular schedule";
}

mod get {
    use super::*;

    pub fn name(args: &mut impl Iterator<Item = String>) -> Result<String> {
        let path = args
            .next()
            .ok_or_else(|| anyhow!("an event category must be specified"))?;
        ensure!(!path.contains('/'), "path must not contain `/`");
        Ok(path)
    }
    pub fn datetime(args: &mut impl Iterator<Item = String>) -> Result<DateTime<Utc>> {
        parse_datetime(
            &args
                .next()
                .ok_or_else(|| anyhow!("a date must be specified"))?,
        )
    }
    pub fn interval(args: &mut impl Iterator<Item = String>) -> Result<TimeDelta> {
        parse_timedelta(
            &args
                .next()
                .ok_or_else(|| anyhow!("an interval must be specified"))?,
        )
    }
}

pub enum Action {
    List,
    New {
        name: String,
        start: DateTime<Utc>,
        every: TimeDelta,
    },
    Step(String),
    Next(String),
}
impl Action {
    pub fn get(args: &mut impl Iterator<Item = String>) -> Result<Self> {
        let Some(action) = args.next() else {
            return Ok(Action::List);
        };
        let action = match &action[..] {
            "list" => Self::List,
            "new" => Self::New {
                name: get::name(args)?,
                start: get::datetime(args)?,
                every: get::interval(args)?,
            },
            "step" => Self::Step(get::name(args)?),
            "next" => Self::Next(get::name(args)?),
            unknown => return Err(anyhow!("unknown action `{unknown}`"))?,
        };
        return Ok(action);
    }
}

#[derive(Clone, Copy, Debug, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct FormattedInterval(pub TimeDelta);
impl Display for FormattedInterval {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}{:02}h{:02}m{:02}s",
            if self.0 < TimeDelta::zero() { "-" } else { "" },
            self.0.num_hours().abs(),
            self.0.num_minutes().abs() % 60,
            self.0.num_seconds().abs() % 60,
        )
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let folder =
        env::var("HOME").expect("`HOME` must exist; cannot access tracker file") + "/.hrtracker/";
    if !fs::exists(&folder)? {
        fs::create_dir(&folder)?;
    }
    let mut argv = env::args().skip(1);
    let action = Action::get(&mut argv)?;
    match action {
        Action::List => {
            for entry in fs::read_dir(folder)? {
                let Ok(entry) = entry else {
                    println!("directory entry error");
                    continue;
                };
                let Ok(schedule) = RegularSchedule::open(entry.path()) else {
                    println!("unable to open {}", entry.path().display());
                    continue;
                };
                let now = Utc::now();
                println!(
                    "schedule `{}`: next at {} (in {}) with interval {}",
                    entry.path().file_name().unwrap().display(),
                    schedule.next,
                    FormattedInterval(schedule.next.signed_duration_since(now)),
                    FormattedInterval(schedule.interval)
                );
            }
        }
        Action::New { name, start, every } => {
            RegularSchedule::create(start, every).save(folder + &name)?;
        }
        Action::Step(name) => {
            let mut schedule = RegularSchedule::open(folder.clone() + &name)?;
            schedule.next += schedule.interval;
            println!(
                "now in {}",
                FormattedInterval(schedule.next.signed_duration_since(Utc::now()))
            );
            schedule.save(folder + &name)?;
        }
        Action::Next(name) => {
            let schedule = RegularSchedule::open(folder + &name)?;
            let delta = schedule.next.signed_duration_since(Utc::now());
            println!("{} (in {})", schedule.next, FormattedInterval(delta));
        }
    }
    Ok(())
}
