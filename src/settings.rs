use objc2_foundation::{NSString, NSUserDefaults};
use serde::{Deserialize, Serialize};

use crate::timezone::TimezoneEntry;

const ENTRIES_KEY: &str = "timezone_entries";

#[derive(Serialize, Deserialize)]
struct StoredEntry {
    label: String,
    iana_id: String,
    #[serde(default)]
    favorite: bool,
}

pub fn load_entries() -> Option<Vec<TimezoneEntry>> {
    let defaults = NSUserDefaults::standardUserDefaults();
    let json = defaults.stringForKey(&NSString::from_str(ENTRIES_KEY))?;
    let stored: Vec<StoredEntry> = serde_json::from_str(&json.to_string()).ok()?;
    if stored.is_empty() {
        return None;
    }
    Some(
        stored
            .into_iter()
            .filter_map(|s| TimezoneEntry::try_new(&s.label, &s.iana_id, s.favorite))
            .collect(),
    )
}

pub fn save_entries(entries: &[TimezoneEntry]) {
    let stored: Vec<StoredEntry> = entries
        .iter()
        .map(|e| StoredEntry {
            label: e.label.clone(),
            iana_id: e.iana_id().to_string(),
            favorite: e.favorite,
        })
        .collect();

    let json = serde_json::to_string(&stored).expect("serializable");
    let defaults = NSUserDefaults::standardUserDefaults();
    unsafe {
        defaults.setObject_forKey(
            Some(&NSString::from_str(&json)),
            &NSString::from_str(ENTRIES_KEY),
        );
    }
}
