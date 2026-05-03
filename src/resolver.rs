use jiff::tz::{Offset, TimeZone};

pub struct ResolvedTimezone {
    pub canonical_id: String,
    pub timezone: TimeZone,
}

pub fn resolve_timezone(input: &str) -> Option<ResolvedTimezone> {
    let raw = input.trim();
    if raw.is_empty() {
        return None;
    }

    if let Ok(timezone) = TimeZone::get(raw) {
        let canonical_id = timezone.iana_name().unwrap_or(raw).to_string();
        return Some(ResolvedTimezone {
            canonical_id,
            timezone,
        });
    }

    let normalized_offset = normalize_offset_id(raw)?;
    let offset = parse_normalized_offset(&normalized_offset)?;
    Some(ResolvedTimezone {
        canonical_id: normalized_offset,
        timezone: TimeZone::fixed(offset),
    })
}

pub fn normalize_offset_id(input: &str) -> Option<String> {
    let upper = input.trim().to_ascii_uppercase();
    let offset = upper
        .strip_prefix("UTC")
        .or_else(|| upper.strip_prefix("GMT"))
        .unwrap_or(&upper);

    if offset.is_empty() {
        return Some("UTC".to_string());
    }

    let (sign, rest) = match offset.as_bytes().first().copied() {
        Some(b'+') => ('+', &offset[1..]),
        Some(b'-') => ('-', &offset[1..]),
        _ => return None,
    };

    let (hours, minutes) = parse_offset_parts(rest)?;
    Some(format!("UTC{sign}{hours:02}:{minutes:02}"))
}

fn parse_normalized_offset(input: &str) -> Option<Offset> {
    if input == "UTC" {
        return Some(Offset::UTC);
    }

    let offset = input.strip_prefix("UTC")?;
    let sign = if offset.starts_with('-') { -1 } else { 1 };
    let (hours, minutes) = parse_offset_parts(&offset[1..])?;
    let seconds = sign * (i32::from(hours) * 60 * 60 + i32::from(minutes) * 60);
    Offset::from_seconds(seconds).ok()
}

fn parse_offset_parts(input: &str) -> Option<(u8, u8)> {
    if input.is_empty() {
        return None;
    }

    let (hours, minutes) = if let Some((hours, minutes)) = input.split_once(':') {
        (parse_one_or_two_digits(hours)?, parse_two_digits(minutes)?)
    } else {
        match input.len() {
            1 | 2 => (parse_one_or_two_digits(input)?, 0),
            4 => (
                parse_two_digits(&input[..2])?,
                parse_two_digits(&input[2..])?,
            ),
            _ => return None,
        }
    };

    if hours > 23 || minutes > 59 {
        return None;
    }

    Some((hours, minutes))
}

fn parse_one_or_two_digits(input: &str) -> Option<u8> {
    if input.len() > 2 {
        return None;
    }
    input.parse().ok()
}

fn parse_two_digits(input: &str) -> Option<u8> {
    if input.len() != 2 {
        return None;
    }
    input.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_iana_id_with_canonical_case() {
        let resolved = resolve_timezone("america/new_york").unwrap();

        assert_eq!(resolved.canonical_id, "America/New_York");
    }

    #[test]
    fn normalizes_utc_offsets() {
        assert_eq!(normalize_offset_id("UTC+1").unwrap(), "UTC+01:00");
        assert_eq!(normalize_offset_id("gmt-8").unwrap(), "UTC-08:00");
        assert_eq!(normalize_offset_id("+0530").unwrap(), "UTC+05:30");
        assert_eq!(normalize_offset_id("-03:30").unwrap(), "UTC-03:30");
    }

    #[test]
    fn rejects_invalid_offsets() {
        assert!(normalize_offset_id("UTC+24").is_none());
        assert!(normalize_offset_id("UTC+12:60").is_none());
        assert!(normalize_offset_id("UTC+123").is_none());
    }

    #[test]
    fn resolves_fixed_offset_timezone() {
        let resolved = resolve_timezone("UTC+5:30").unwrap();

        assert_eq!(resolved.canonical_id, "UTC+05:30");
        assert_eq!(
            resolved.timezone.to_fixed_offset().unwrap().seconds(),
            19_800
        );
    }

    #[test]
    fn rejects_empty_and_whitespace() {
        assert!(resolve_timezone("").is_none());
        assert!(resolve_timezone("   ").is_none());
    }

    #[test]
    fn normalizes_bare_utc_and_gmt() {
        assert_eq!(normalize_offset_id("UTC").unwrap(), "UTC");
        assert_eq!(normalize_offset_id("gmt").unwrap(), "UTC");
        assert_eq!(normalize_offset_id("  utc  ").unwrap(), "UTC");
    }

    #[test]
    fn rejects_nonsense_input() {
        assert!(resolve_timezone("Not/A/Timezone").is_none());
        assert!(normalize_offset_id("hello").is_none());
    }
}
