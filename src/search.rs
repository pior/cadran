use jiff::tz;

pub struct TimezoneSearchEntry {
    pub iana_id: String,
    pub city: String,
    pub display: String,
}

pub struct TimezoneSearch {
    entries: Vec<TimezoneSearchEntry>,
}

impl TimezoneSearch {
    pub fn new() -> Self {
        let mut entries = Vec::new();
        for name in tz::db().available() {
            let name = name.to_string();
            let city = extract_city(&name);
            let display = format!("{city} \u{2014} {name}");
            entries.push(TimezoneSearchEntry {
                iana_id: name,
                city,
                display,
            });
        }
        entries.sort_by(|a, b| a.city.cmp(&b.city));
        Self { entries }
    }

    /// Returns combo box items: city-prefixed labels for city search,
    /// plus raw IANA IDs for direct IANA prefix search.
    pub fn combo_items(&self) -> Vec<&str> {
        let mut items: Vec<&str> = Vec::with_capacity(self.entries.len() * 2);
        // City-prefixed entries first (sorted by city)
        for e in &self.entries {
            items.push(&e.display);
        }
        // Raw IANA IDs (sorted alphabetically by ID)
        let mut iana_ids: Vec<&str> = self.entries.iter().map(|e| e.iana_id.as_str()).collect();
        iana_ids.sort();
        items.extend(iana_ids);
        items
    }
}

/// Extract the IANA ID from a display label like "Tokyo — Asia/Tokyo".
/// If the input is already a plain IANA ID (contains '/'), return it as-is.
pub fn iana_id_from_display(display: &str) -> &str {
    if let Some((_city, iana)) = display.split_once(" \u{2014} ") {
        iana.trim()
    } else {
        display.trim()
    }
}

fn extract_city(iana_id: &str) -> String {
    iana_id
        .rsplit('/')
        .next()
        .unwrap_or(iana_id)
        .replace('_', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combo_items_contain_city_and_iana() {
        let search = TimezoneSearch::new();
        let items = search.combo_items();
        // City-prefixed entries
        assert!(
            items.iter().any(|l| l.starts_with("Tokyo")),
            "expected a combo item starting with Tokyo"
        );
        // Raw IANA IDs
        assert!(
            items.iter().any(|l| *l == "Asia/Tokyo"),
            "expected Asia/Tokyo as a raw IANA combo item"
        );
        assert!(
            items.iter().any(|l| *l == "Europe/Paris"),
            "expected Europe/Paris as a raw IANA combo item"
        );
    }

    #[test]
    fn iana_id_from_display_label() {
        assert_eq!(
            iana_id_from_display("Tokyo \u{2014} Asia/Tokyo"),
            "Asia/Tokyo"
        );
        assert_eq!(
            iana_id_from_display("New York \u{2014} America/New_York"),
            "America/New_York"
        );
    }

    #[test]
    fn iana_id_from_raw_input() {
        assert_eq!(iana_id_from_display("Europe/Paris"), "Europe/Paris");
        assert_eq!(iana_id_from_display("Asia/Tokyo"), "Asia/Tokyo");
    }

    #[test]
    fn extract_city_from_iana() {
        assert_eq!(extract_city("Asia/Tokyo"), "Tokyo");
        assert_eq!(extract_city("America/New_York"), "New York");
        assert_eq!(extract_city("Europe/Paris"), "Paris");
        assert_eq!(extract_city("US/Eastern"), "Eastern");
    }
}
