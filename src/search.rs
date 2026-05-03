use jiff::{tz, Timestamp};

use crate::resolver;

pub struct TimezoneSearchEntry {
    pub iana_id: String,
    pub city: String,
    tags: Vec<String>,
    fuzzy_tags: Vec<String>,
}

pub struct TimezoneSearch {
    entries: Vec<TimezoneSearchEntry>,
}

impl TimezoneSearch {
    pub fn new() -> Self {
        let mut entries = Vec::new();
        let now = Timestamp::now();
        for name in tz::db().available() {
            let name = name.to_string();
            let city = extract_city(&name);
            let mut tags = vec![name.to_lowercase(), city.to_lowercase()];
            let mut fuzzy_tags = vec![compact_match_key(&name), compact_match_key(&city)];
            if let Some(initials) = initials_match_key(&city) {
                fuzzy_tags.push(initials);
            }
            if let Ok(timezone) = tz::TimeZone::get(&name) {
                let abbreviation = timezone.to_offset_info(now).abbreviation().to_lowercase();
                tags.push(abbreviation.clone());
                if is_alpha_abbreviation(&abbreviation) {
                    if let Some(family) = abbreviation_family(&abbreviation) {
                        tags.push(family.to_string());
                    }
                }
            }
            entries.push(TimezoneSearchEntry {
                iana_id: name,
                city,
                tags,
                fuzzy_tags,
            });
        }
        entries.sort_by(|a, b| a.city.cmp(&b.city));
        Self { entries }
    }

    /// Returns combo box items while keeping city and abbreviation aliases
    /// searchable through `completions_for`.
    pub fn combo_items(&self) -> Vec<String> {
        let mut items: Vec<String> =
            Vec::with_capacity(self.entries.len() + offset_suggestions().len());
        items.extend(offset_suggestions());

        let mut iana_ids: Vec<String> = self.entries.iter().map(|e| e.iana_id.clone()).collect();
        iana_ids.sort();
        items.extend(iana_ids);
        items
    }

    pub fn completions_for(&self, query: &str) -> Vec<String> {
        let query = query.trim().to_lowercase();
        if query.is_empty() {
            return self.combo_items();
        }

        let mut items = Vec::new();
        if let Some(offset) = resolver::normalize_offset_id(&query) {
            items.push(offset);
        }
        items.extend(
            offset_suggestions()
                .into_iter()
                .filter(|item| item.to_lowercase().starts_with(&query)),
        );
        items.extend(
            self.entries
                .iter()
                .filter(|entry| entry.matches_query(&query))
                .map(|entry| entry.iana_id.clone()),
        );

        items.dedup();
        items
    }
}

impl TimezoneSearchEntry {
    fn matches_query(&self, query: &str) -> bool {
        let fuzzy_query = compact_match_key(query);
        self.tags.iter().any(|tag| tag.contains(query))
            || (!fuzzy_query.is_empty()
                && self.fuzzy_tags.iter().any(|tag| {
                    tag.contains(&fuzzy_query) || is_subsequence(&fuzzy_query, tag)
                }))
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

fn abbreviation_family(abbreviation: &str) -> Option<&'static str> {
    let bytes = abbreviation.as_bytes();
    if bytes.len() < 3 || bytes.last().copied() != Some(b't') {
        return None;
    }
    let marker = bytes[bytes.len() - 2];
    if marker != b's' && marker != b'd' {
        return None;
    }

    match &abbreviation[..abbreviation.len() - 2] {
        "a" => Some("at"),
        "ac" => Some("act"),
        "ae" => Some("aet"),
        "b" => Some("bt"),
        "c" => Some("ct"),
        "ce" => Some("cet"),
        "e" => Some("et"),
        "ee" => Some("eet"),
        "g" => Some("gt"),
        "h" => Some("ht"),
        "i" => Some("it"),
        "j" => Some("jt"),
        "m" => Some("mt"),
        "n" => Some("nt"),
        "p" => Some("pt"),
        "w" => Some("wt"),
        "we" => Some("wet"),
        _ => None,
    }
}

fn is_alpha_abbreviation(abbreviation: &str) -> bool {
    abbreviation.bytes().all(|byte| byte.is_ascii_alphabetic())
}

fn offset_suggestions() -> Vec<String> {
    let mut suggestions = vec!["UTC".to_string()];
    for minutes in (-12_i32 * 60..=14_i32 * 60).step_by(15) {
        if minutes == 0 {
            continue;
        }
        let sign = if minutes < 0 { '-' } else { '+' };
        let absolute = minutes.abs();
        suggestions.push(format!(
            "UTC{sign}{:02}:{:02}",
            absolute / 60,
            absolute % 60
        ));
    }
    suggestions
}

fn extract_city(iana_id: &str) -> String {
    iana_id
        .rsplit('/')
        .next()
        .unwrap_or(iana_id)
        .replace('_', " ")
}

fn compact_match_key(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}

fn initials_match_key(value: &str) -> Option<String> {
    let initials: String = value
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter_map(|word| word.chars().next())
        .flat_map(|ch| ch.to_lowercase())
        .collect();

    if initials.len() > 1 {
        Some(initials)
    } else {
        None
    }
}

fn is_subsequence(needle: &str, haystack: &str) -> bool {
    let mut chars = needle.chars();
    let Some(mut current) = chars.next() else {
        return true;
    };

    for candidate in haystack.chars() {
        if candidate == current {
            match chars.next() {
                Some(next) => current = next,
                None => return true,
            }
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combo_items_contain_city_and_iana() {
        let search = TimezoneSearch::new();
        let items = search.combo_items();

        assert!(
            items.iter().any(|l| l == "Asia/Tokyo"),
            "expected Asia/Tokyo as a raw IANA combo item"
        );
        assert!(
            items.iter().any(|l| l == "Europe/Paris"),
            "expected Europe/Paris as a raw IANA combo item"
        );
    }

    #[test]
    fn combo_items_contain_offset_suggestions() {
        let search = TimezoneSearch::new();
        let items = search.combo_items();

        assert!(items.iter().any(|l| l == "UTC-12:00"));
        assert!(items.iter().any(|l| l == "UTC+01:00"));
        assert!(items.iter().any(|l| l == "UTC+05:45"));
        assert!(items.iter().any(|l| l == "UTC+14:00"));
        assert!(items.iter().any(|l| l == "UTC-08:00"));
    }

    #[test]
    fn combo_items_contain_explicit_family_suggestions() {
        let search = TimezoneSearch::new();
        let items = search.completions_for("ET");

        assert!(
            items.iter().any(|l| l == "America/New_York"),
            "expected an explicit ET suggestion for America/New_York"
        );
    }

    #[test]
    fn combo_items_contain_explicit_abbreviation_suggestions() {
        let search = TimezoneSearch::new();
        let items = search.completions_for("JST");

        assert!(
            items.iter().any(|l| l == "Asia/Tokyo"),
            "expected an explicit JST suggestion for Asia/Tokyo"
        );
    }

    #[test]
    fn completions_include_offset_and_family_matches() {
        let search = TimezoneSearch::new();

        let utc_matches = search.completions_for("UTC");
        assert_eq!(utc_matches[0], "UTC");
        assert!(utc_matches.iter().any(|l| l == "UTC+14:00"));

        assert_eq!(search.completions_for("utc+1")[0], "UTC+01:00");
        assert!(
            search
                .completions_for("ET")
                .iter()
                .any(|l| l == "America/New_York"),
            "expected ET completions to include America/New_York"
        );
        assert!(
            search
                .completions_for("ET")
                .iter()
                .any(|l| l == "America/Detroit"),
            "expected ET completions to include IANA/city text matches"
        );
    }

    #[test]
    fn completions_return_iana_ids_for_city_matches() {
        let search = TimezoneSearch::new();

        assert!(
            search
                .completions_for("Paris")
                .iter()
                .any(|l| l == "Europe/Paris"),
            "expected city search to return the matching IANA ID"
        );
        assert!(
            search
                .completions_for("new")
                .iter()
                .any(|l| l == "America/New_York"),
            "expected New York search to return the matching IANA ID"
        );
    }

    #[test]
    fn completions_include_fuzzy_city_matches() {
        let search = TimezoneSearch::new();

        assert!(
            search
                .completions_for("newyork")
                .iter()
                .any(|l| l == "America/New_York"),
            "expected compact city search to return America/New_York"
        );
        assert!(
            search
                .completions_for("ny")
                .iter()
                .any(|l| l == "America/New_York"),
            "expected city initials to match multi-word city names"
        );
        assert!(
            search
                .completions_for("rga")
                .iter()
                .any(|l| l == "America/Argentina/Rio_Gallegos"),
            "expected subsequence search to match compact city names"
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
    fn list_all_available_timezones() {
        let mut available: Vec<_> = jiff::tz::db().available().collect();
        available.sort();
        for name in available {
            println!("ALL_TZ: {}", name.as_str());
        }
    }
}
