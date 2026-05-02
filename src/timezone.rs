use jiff::tz::TimeZone;
use jiff::Zoned;

pub struct TimezoneEntry {
    pub label: String,
    tz: TimeZone,
    iana_id: String,
}

pub struct FormattedEntry {
    pub label: String,
    pub time: String,
    pub relative_day: &'static str,
}

impl TimezoneEntry {
    pub fn new(label: &str, iana_id: &str) -> Self {
        let tz = TimeZone::get(iana_id).expect("valid IANA timezone ID");
        Self {
            label: label.to_string(),
            tz,
            iana_id: iana_id.to_string(),
        }
    }

    pub fn try_new(label: &str, iana_id: &str) -> Option<Self> {
        let tz = TimeZone::get(iana_id).ok()?;
        Some(Self {
            label: label.to_string(),
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
    fn test_relative_day_label() {
        let utc = TimeZone::UTC;
        let tokyo = TimeZone::get("Asia/Tokyo").unwrap();
        let ny = TimeZone::get("America/New_York").unwrap();

        // Local time: 2024-05-02 12:00:00 UTC
        let now = date(2024, 5, 2).at(12, 0, 0, 0).to_zoned(utc).unwrap();

        // Target Tokyo: 2024-05-02 21:00:00 (Same day)
        let target_tokyo = now.with_time_zone(tokyo.clone());
        assert_eq!(relative_day_label(&now, &target_tokyo), "Today");

        // Target NY: 2024-05-02 08:00:00 (Same day)
        let target_ny = now.with_time_zone(ny.clone());
        assert_eq!(relative_day_label(&now, &target_ny), "Today");

        // Late night UTC: 2024-05-02 22:00:00 UTC
        let late_now = date(2024, 5, 2).at(22, 0, 0, 0).to_zoned(TimeZone::UTC).unwrap();
        // Tokyo is now 2024-05-03 07:00:00 (Tomorrow)
        let target_tokyo_tmw = late_now.with_time_zone(tokyo);
        assert_eq!(relative_day_label(&late_now, &target_tokyo_tmw), "Tomorrow");

        // Early morning UTC: 2024-05-02 02:00:00 UTC
        let early_now = date(2024, 5, 2).at(2, 0, 0, 0).to_zoned(TimeZone::UTC).unwrap();
        // NY is still 2024-05-01 22:00:00 (Yesterday)
        let target_ny_yest = early_now.with_time_zone(ny);
        assert_eq!(relative_day_label(&early_now, &target_ny_yest), "Yesterday");
    }
}

pub fn default_entries() -> Vec<TimezoneEntry> {
    vec![
        TimezoneEntry::new("Sam", "Europe/London"),
        TimezoneEntry::new("Mika", "Asia/Tokyo"),
        TimezoneEntry::new("Ana", "America/New_York"),
        TimezoneEntry::new("Priya", "Asia/Kolkata"),
        TimezoneEntry::new("Leo", "Australia/Sydney"),
    ]
}
