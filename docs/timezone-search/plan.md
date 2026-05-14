# Timezone Search Improvements

## Objective
Improve the timezone search functionality to support additional timezone identifiers beyond standard IANA IDs.

Specifically, we want to support:
- Short timezone identifiers (e.g., "PT" for Pacific Time).
- Offset-based identifiers (e.g., "UTC+4", "GMT-8").
- Major cities that are not primary IANA IDs (e.g., "San Francisco").

## Research Findings
- The current implementation uses the `jiff` crate, which primarily supports IANA identifiers (e.g., `America/Los_Angeles`).
- `jiff` does not support parsing abbreviations like "PT" or "EST" because they are ambiguous.
- `jiff` does not support parsing offset strings like "UTC+4" directly into a `TimeZone` object via name lookup.
- "San Francisco" is not present in the IANA database (it maps to `America/Los_Angeles`).

## Proposed Solution

### 1. Unified Timezone Resolver
Introduce a resolver that can turn various strings into a `jiff::tz::TimeZone`.

The resolver will handle:
- **IANA IDs:** Directly handled by `jiff::tz::TimeZone::get`.
- **Offsets:** Support for strings like `UTC+4`, `UTC-8`, `GMT+5`, `+04:00`, `-0800`.
- **Essential Aliases:** A curated, non-brittle mapping of common abbreviations and major cities to their canonical IANA IDs.

#### Essential Aliases Mapping (Draft)
| Input | IANA ID |
| :--- | :--- |
| PT | America/Los_Angeles |
| ET | America/New_York |
| CT | America/Chicago |
| MT | America/Denver |
| San Francisco | America/Los_Angeles |
| Seattle | America/Los_Angeles |
| Washington DC | America/New_York |

### 2. Search Suggestions
Enhance `TimezoneSearch` to include these essential aliases in the combo box items so users can discover them.

### 3. Data Persistence
Ensure that `TimezoneEntry` can be reconstructed from these identifiers.
- For IANA IDs and Aliases: Store the IANA ID.
- For Offsets: Store a normalized offset string (e.g., `UTC+04:00`) that the resolver can parse back.

## Implementation Steps

1. **Create `src/resolver.rs`:**
    - Implement `resolve_timezone(input: &str) -> Option<TimeZone>`.
    - Implement offset parsing logic.
    - Define the static alias map.

2. **Update `TimezoneEntry`:**
    - Update `try_new` to use the resolver.
    - If it's an offset, store it in a way that remains valid.

3. **Update `TimezoneSearch`:**
    - Inject the essential aliases into the list of searchable entries.
    - Ensure they are sorted and displayed appropriately.

4. **Testing:**
    - Unit tests for the resolver with various inputs.
    - Integration test for search suggestions.

## Alternatives Considered
- **Using a large city database:** Rejected as potentially brittle and increasing complexity/binary size beyond the project's needs.
- **Ambiguity Handling:** For ambiguous abbreviations like "CST", we will default to the most likely candidate (Central Standard Time in US) or omit it to avoid confusion, prioritizing reliability.
