use jiff::tz::TimeZone;
use jiff::Zoned;

pub struct TimezoneEntry {
    pub label: String,
    pub city: String,
    tz: TimeZone,
    iana_id: String,
}

pub struct FormattedEntry {
    pub label: String,
    pub city: String,
    pub time: String,
    pub relative_day: &'static str,
}

impl TimezoneEntry {
    pub fn new(label: &str, city: &str, iana_id: &str) -> Self {
        let tz = TimeZone::get(iana_id).expect("valid IANA timezone ID");
        Self {
            label: label.to_string(),
            city: city.to_string(),
            tz,
            iana_id: iana_id.to_string(),
        }
    }

    pub fn try_new(label: &str, city: &str, iana_id: &str) -> Option<Self> {
        let tz = TimeZone::get(iana_id).ok()?;
        Some(Self {
            label: label.to_string(),
            city: city.to_string(),
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
            city: self.city.clone(),
            time,
            relative_day,
        }
    }
}

fn relative_day_label(local: &Zoned, other: &Zoned) -> &'static str {
    let local_date = local.date();
    let other_date = other.date();

    if other_date == local_date {
        "Today"
    } else if other_date == local_date.tomorrow().expect("valid date") {
        "Tomorrow"
    } else if other_date == local_date.yesterday().expect("valid date") {
        "Yesterday"
    } else {
        ""
    }
}

pub fn default_entries() -> Vec<TimezoneEntry> {
    vec![
        TimezoneEntry::new("Sam", "London", "Europe/London"),
        TimezoneEntry::new("Mika", "Tokyo", "Asia/Tokyo"),
        TimezoneEntry::new("Ana", "New York", "America/New_York"),
        TimezoneEntry::new("Priya", "Bangalore", "Asia/Kolkata"),
        TimezoneEntry::new("Leo", "Sydney", "Australia/Sydney"),
    ]
}
