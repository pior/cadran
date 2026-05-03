use jiff::tz::TimeZone;
use jiff::Zoned;

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
        let tz = TimeZone::get(iana_id).expect("valid IANA timezone ID");
        Self {
            label: label.to_string(),
            favorite,
            tz,
            iana_id: iana_id.to_string(),
        }
    }

    pub fn try_new(label: &str, iana_id: &str, favorite: bool) -> Option<Self> {
        let tz = TimeZone::get(iana_id).ok()?;
        Some(Self {
            label: label.to_string(),
            favorite,
            tz,
            iana_id: iana_id.to_string(),
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
    } else if date_target > date_now {
        "Tomorrow"
    } else {
        "Yesterday"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::date;
    use jiff::tz::TimeZone;

    #[test]
    fn test_jiff_tz_get() {
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
