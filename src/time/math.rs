//! Date math: durations added and subtracted, calendar steps, working days.

use super::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Step {
    /// Nanoseconds per unit.
    Exact(i128),
    Days(i64),
    /// Working days: Monday to Friday.
    WorkDays,
    Months(i64),
}

pub(super) fn step(symbol: &str) -> Option<Step> {
    Some(match symbol {
        "ns" => Step::Exact(1),
        "us" => Step::Exact(1_000),
        "ms" => Step::Exact(1_000_000),
        "s" => Step::Exact(NS),
        "min" => Step::Exact(60 * NS),
        "h" => Step::Exact(3600 * NS),
        "d" => Step::Days(1),
        "wk" => Step::Days(7),
        WORKDAY => Step::WorkDays,
        "mo" => Step::Months(1),
        "yr" => Step::Months(12),
        _ => return None,
    })
}

/// `instant` plus `amount` (a decimal; whole for calendar units) of `symbol`, `sign` = ±1.
pub(super) fn add(instant: Instant, amount: &str, symbol: &str, sign: i128) -> Result<Instant, TimeError> {
    let step = step(symbol).ok_or_else(|| TimeError::Usage(format!("`{symbol}` is not a time unit")))?;
    let whole =
        || -> Result<i64, TimeError> { amount.parse::<i64>().map_err(|_| TimeError::Usage(format!("`{amount}{symbol}`: calendar steps need a whole number"))) };
    match step {
        Step::Exact(size) => {
            let delta = scaled(amount, size).ok_or(TimeError::Range)?;
            Instant { ns: instant.ns.checked_add(sign * delta).ok_or(TimeError::Range)?, ..instant }.in_range()
        }
        Step::Days(days) => {
            let c = instant.civil();
            let n = whole()?.checked_mul(days).ok_or(TimeError::Range)? * sign as i64;
            let (year, month, day) = civil_from_days(days_from_civil(c.year, c.month, c.day).checked_add(n).ok_or(TimeError::Range)?);
            from_civil(&Civil { year, month, day, ..c }, instant.zone)
        }
        Step::WorkDays => {
            let c = instant.civil();
            let n = whole()?.checked_mul(sign as i64).ok_or(TimeError::Range)?;
            let days = days_from_civil(c.year, c.month, c.day);
            let days = add_workdays(days, n).ok_or(TimeError::Range)?;
            let (year, month, day) = civil_from_days(days);
            from_civil(&Civil { year, month, day, ..c }, instant.zone)
        }
        Step::Months(months) => {
            let c = instant.civil();
            let n = whole()?.checked_mul(months).ok_or(TimeError::Range)? * sign as i64;
            let total = (c.year * 12 + i64::from(c.month) - 1).checked_add(n).ok_or(TimeError::Range)?;
            let (year, month) = (total.div_euclid(12), total.rem_euclid(12) as u32 + 1);
            if !(MIN_YEAR..=MAX_YEAR).contains(&year) {
                return Err(TimeError::Range);
            }
            let day = c.day.min(days_in_month(year, month));
            from_civil(&Civil { year, month, day, ..c }, instant.zone)
        }
    }
}

/// The working-day step (`5wd`, Monday to Friday); not a duration, so not in the unit table.
pub(super) const WORKDAY: &str = "wd";
pub(super) const WORKDAY_NAMES: [&str; 5] = [WORKDAY, "workday", "workdays", "weekday", "weekdays"];

pub(super) fn is_weekend(days: i64) -> bool {
    weekday(days) >= 6
}

/// `days` plus `n` working days (`WORKDAY` in spreadsheets): each step lands on a Monday–Friday,
/// weekends are skipped. From a weekend day, `+1wd` is the next Monday and `-1wd` the previous
/// Friday; `0wd` is the day itself.
pub(super) fn add_workdays(days: i64, n: i64) -> Option<i64> {
    if n == 0 {
        return Some(days);
    }
    let sign = n.signum();
    // From a weekend, start from the working day before it, in the direction of travel.
    let mut d = days;
    while is_weekend(d) {
        d -= sign;
    }
    // Whole weeks of 5 working days, then the rest one working day at a time.
    d = d.checked_add((n / 5).checked_mul(7)?)?;
    for _ in 0..(n % 5).abs() {
        d += sign;
        while is_weekend(d) {
            d += sign;
        }
    }
    Some(d)
}

/// Working days from day `from` to day `to`: those in `(from, to]` (negative when `to` is earlier),
/// so that `from + n wd = to` when `to` is a working day.
pub(super) fn workdays_between(from: i64, to: i64) -> i64 {
    // Working days before day `d`, counted from Monday 1969-12-29 (day -3).
    let before = |d: i64| {
        let wd = i64::from(weekday(d));
        (d - wd + 1 + 3).div_euclid(7) * 5 + (wd - 1).min(5)
    };
    before(to + 1) - before(from + 1)
}
