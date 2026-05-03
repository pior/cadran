use jiff::tz::TimeZone;
use jiff::Zoned;

use crate::resolver;

pub struct TimezoneEntry {
    pub label: String,
    pub favorite: bool,
    tz: TimeZone,
    iana_id: String,
}

pub struct FormattedEntry {
    pub label: String,
    pub time: String,
    pub relative_day: &'static str,
}

impl TimezoneEntry {
    pub fn new(label: &str, iana_id: &str, favorite: bool) -> Self {
        let resolved = resolver::resolve_timezone(iana_id).expect("valid timezone ID");
        Self {
            label: label.to_string(),
            favorite,
            tz: resolved.timezone,
            iana_id: resolved.canonical_id,
        }
    }

    pub fn try_new(label: &str, iana_id: &str, favorite: bool) -> Option<Self> {
        let resolved = resolver::resolve_timezone(iana_id)?;
        Some(Self {
            label: label.to_string(),
            favorite,
            tz: resolved.timezone,
            iana_id: resolved.canonical_id,
        })
    }

    pub fn iana_id(&self) -> &str {
        &self.iana_id
    }

    pub fn format(&self, now: &Zoned) -> FormattedEntry {
        let converted = now.with_time_zone(self.tz.clone());
        let time = converted.strftime("%H:%M").to_string();
        let relative_day = relative_day_label(now, &converted);

        FormattedEntry {
            label: self.label.clone(),
            time,
            relative_day,
        }
    }
}

fn relative_day_label(now: &Zoned, target: &Zoned) -> &'static str {
    let date_now = now.date();
    let date_target = target.date();

    if date_target == date_now {
        "Today"
    } else if date_now
        .tomorrow()
        .is_ok_and(|tomorrow| date_target == tomorrow)
    {
        "Tomorrow"
    } else if date_now
        .yesterday()
        .is_ok_and(|yesterday| date_target == yesterday)
    {
        "Yesterday"
    } else {
        ""
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;

    #[test]
    fn jiff_accepts_common_timezone_names() {
        let cases = vec![
            "UTC",
            "UTC+4",
            "UTC-8",
            "GMT+4",
            "GMT-8",
            "PT",
            "PST",
            "PDT",
            "America/Los_Angeles",
            "US/Pacific",
        ];
        for case in cases {
            let res = jiff::tz::TimeZone::get(case);
            println!("GET '{}': {}", case, res.is_ok());
        }
    }

    #[test]
    fn relative_day_is_today_when_dates_match() {
        let now = date(2024, 5, 3)
            .at(12, 0, 0, 0)
            .in_tz("Europe/Paris")
            .unwrap();
        let target = now.with_time_zone(TimeZone::get("America/New_York").unwrap());

        assert_eq!(relative_day_label(&now, &target), "Today");
    }

    #[test]
    fn relative_day_is_tomorrow_across_month_and_year_boundary() {
        let now = date(2024, 12, 31)
            .at(23, 30, 0, 0)
            .in_tz("Europe/Paris")
            .unwrap();
        let target = now.with_time_zone(TimeZone::get("Asia/Tokyo").unwrap());

        assert_eq!(relative_day_label(&now, &target), "Tomorrow");
    }

    #[test]
    fn relative_day_is_tomorrow_for_paris_to_adelaide_evening() {
        let now = date(2026, 5, 3)
            .at(17, 30, 0, 0)
            .in_tz("Europe/Paris")
            .unwrap();
        let target = now.with_time_zone(TimeZone::get("Australia/Adelaide").unwrap());

        assert_eq!(target.strftime("%H:%M").to_string(), "01:00");
        assert_eq!(relative_day_label(&now, &target), "Tomorrow");
    }

    #[test]
    fn relative_day_is_yesterday_across_month_and_year_boundary() {
        let now = date(2025, 1, 1)
            .at(0, 30, 0, 0)
            .in_tz("Europe/Paris")
            .unwrap();
        let target = now.with_time_zone(TimeZone::get("America/New_York").unwrap());

        assert_eq!(relative_day_label(&now, &target), "Yesterday");
    }

    #[test]
    fn relative_day_is_blank_for_non_adjacent_dates() {
        let now = date(2025, 1, 1)
            .at(12, 0, 0, 0)
            .in_tz("Europe/Paris")
            .unwrap();
        let target = date(2025, 1, 3)
            .at(12, 0, 0, 0)
            .in_tz("Europe/Paris")
            .unwrap();

        assert_eq!(relative_day_label(&now, &target), "");
    }
}

pub fn default_entries() -> Vec<TimezoneEntry> {
    vec![
        TimezoneEntry::new("Sam", "Europe/London", true),
        TimezoneEntry::new("Mika", "Asia/Tokyo", false),
        TimezoneEntry::new("Ana", "America/New_York", false),
        TimezoneEntry::new("Priya", "Asia/Kolkata", false),
        TimezoneEntry::new("Leo", "Australia/Sydney", false),
    ]
}
