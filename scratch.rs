use chrono::{DateTime, Utc};
use std::collections::HashMap;

struct GraphTimelineItem {
    text: String,
    occurred_at: String,
}

fn compress(items: &[GraphTimelineItem]) -> Vec<&GraphTimelineItem> {
    let mut compressed = Vec::new();
    let mut last_seen: HashMap<String, DateTime<Utc>> = HashMap::new();

    for item in items {
        let current_time = DateTime::parse_from_rfc3339(&item.occurred_at)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        let text = &item.text;

        if let Some(last_time) = last_seen.get(text) {
            let duration = current_time.signed_duration_since(*last_time);
            if duration.num_seconds() < 60 {
                // Skip this item to compress the timeline
                continue;
            }
        }

        last_seen.insert(text.clone(), current_time);
        compressed.push(item);
    }
    
    compressed
}
