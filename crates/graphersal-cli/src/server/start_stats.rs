//! The block `--server` prints on stderr at start, after the graph is in memory: the load time
//! and the graph's statistics, from the same data as the playground's Statistics tab
//! ([`graphersal_session::Session::statistics`]: `{statistics, memory, limits}`; the limits
//! have their own `Limits` line in the banner).

use std::time::Duration;

use graphersal_session::store::bytes_text;
use serde_json::Value;

/// How many labels of each kind the block names (the largest first); the rest is `+N more`.
const MAX_LABELS: usize = 10;

/// The lines of the start block: load time with the totals, the vertex and edge labels (when
/// there are any), the graph's memory and the process heap (`live_heap`, `0` = unknown), the
/// compressed values (when there are any). `replayed` is the WAL commits a Store open replayed
/// onto its snapshot (`None`: not a Store).
pub(super) fn lines(
    statistics: &Value,
    load_time: Duration,
    replayed: Option<u64>,
    live_heap: u64,
) -> Vec<String> {
    let mut lines = Vec::new();
    let stats = &statistics["statistics"];
    let how = match replayed {
        Some(0) => " (snapshot, no WAL commits to replay)".to_string(),
        Some(1) => " (snapshot + 1 WAL commit replayed)".to_string(),
        Some(n) => format!(" (snapshot + {} WAL commits replayed)", count_text(n)),
        None => String::new(),
    };
    let totals = if stats.is_object() {
        format!(
            "{} vertices, {} edges",
            count_text(stats["vertex_count"].as_u64().unwrap_or(0)),
            count_text(stats["edge_count"].as_u64().unwrap_or(0))
        )
    } else {
        "no statistics".to_string()
    };
    lines.push(format!(
        "Loaded in {}{how}: {totals}",
        duration_text(load_time)
    ));
    for (title, key, total) in [
        ("Vertex labels", "vertex_labels", "vertex_label_total"),
        ("Edge labels", "edge_labels", "edge_label_total"),
    ] {
        if let Some(line) = labels_line(title, &stats[key], stats[total].as_u64()) {
            lines.push(line);
        }
    }
    let memory = &statistics["memory"];
    let mut parts = Vec::new();
    if memory.is_object() {
        let bytes = |key: &str| bytes_text(memory[key].as_u64().unwrap_or(0));
        // The property indexes only when the graph has some (declared, never automatic).
        let property_indexes = match memory["property_index_bytes"].as_u64() {
            Some(n) if n > 0 => format!(", property indexes {}", bytes_text(n)),
            _ => String::new(),
        };
        parts.push(format!(
            "graph {} (structure {}, indexes {}, properties {}{property_indexes})",
            bytes("total_bytes"),
            bytes("structure_bytes"),
            bytes("index_bytes"),
            bytes("property_bytes")
        ));
    }
    if live_heap > 0 {
        parts.push(format!("process heap {}", bytes_text(live_heap)));
    }
    if !parts.is_empty() {
        lines.push(format!("Memory: {}", parts.join("; ")));
    }
    let compressed = memory["compressed_values"].as_u64().unwrap_or(0);
    if compressed > 0 {
        let bytes = |key: &str| bytes_text(memory[key].as_u64().unwrap_or(0));
        lines.push(format!(
            "Compressed: {} values, {} stored as {} (dictionaries {})",
            count_text(compressed),
            bytes("compressed_plain_bytes"),
            bytes("compressed_stored_bytes"),
            bytes("compression_dictionary_bytes")
        ));
    }
    let deduped = memory["deduped_values"].as_u64().unwrap_or(0);
    if deduped > 0 {
        let bytes = |key: &str| bytes_text(memory[key].as_u64().unwrap_or(0));
        lines.push(format!(
            "Deduplicated: {} values, {} shared as {}",
            count_text(deduped),
            bytes("deduped_plain_bytes"),
            bytes("dedup_table_bytes")
        ));
    }
    lines
}

/// `Vertex labels (12): person 4,000, software 2, ... +2 more`; `None` without labels.
fn labels_line(title: &str, labels: &Value, total: Option<u64>) -> Option<String> {
    let labels = labels.as_array().filter(|labels| !labels.is_empty())?;
    let total = total.unwrap_or(labels.len() as u64);
    let shown: Vec<String> = labels
        .iter()
        .take(MAX_LABELS)
        .map(|label| {
            format!(
                "{} {}",
                label["label"].as_str().unwrap_or("?"),
                count_text(label["count"].as_u64().unwrap_or(0))
            )
        })
        .collect();
    let more = total.saturating_sub(shown.len() as u64);
    let more = if more > 0 {
        format!(", +{} more", count_text(more))
    } else {
        String::new()
    };
    Some(format!(
        "{title} ({}): {}{more}",
        count_text(total),
        shown.join(", ")
    ))
}

/// A count with thousands separators: `1234567` -> `1,234,567`.
fn count_text(n: u64) -> String {
    let digits = n.to_string();
    let mut text = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, digit) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            text.push(',');
        }
        text.push(digit);
    }
    text
}

/// A load time: `0.42 ms`, `12.3 ms`, `1.24 s`.
fn duration_text(duration: Duration) -> String {
    let ms = duration.as_secs_f64() * 1000.0;
    if ms < 1.0 {
        format!("{ms:.2} ms")
    } else if ms < 1000.0 {
        format!("{ms:.1} ms")
    } else {
        format!("{:.2} s", ms / 1000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn counts_get_thousands_separators() {
        assert_eq!(count_text(0), "0");
        assert_eq!(count_text(999), "999");
        assert_eq!(count_text(1000), "1,000");
        assert_eq!(count_text(1_234_567), "1,234,567");
    }

    #[test]
    fn durations_are_human() {
        assert_eq!(duration_text(Duration::from_micros(420)), "0.42 ms");
        assert_eq!(duration_text(Duration::from_micros(12_340)), "12.3 ms");
        assert_eq!(duration_text(Duration::from_millis(1_240)), "1.24 s");
    }

    #[test]
    fn the_block_names_totals_labels_memory_and_compression() {
        let labels: Vec<Value> = (0..12)
            .map(|i| json!({"label": format!("l{i}"), "count": 1000 - i}))
            .collect();
        let statistics = json!({
            "statistics": {
                "vertex_count": 12_000, "edge_count": 3,
                "vertex_labels": labels, "vertex_label_total": 12,
                "edge_labels": [{"label": "knows", "count": 3}], "edge_label_total": 1,
            },
            "memory": {
                "structure_bytes": 1024, "index_bytes": 512, "property_bytes": 2048,
                "total_bytes": 3584, "compressed_values": 2, "compressed_plain_bytes": 4096,
                "compressed_stored_bytes": 1024, "compression_dictionary_bytes": 0,
            },
        });
        let lines = lines(&statistics, Duration::from_millis(5), Some(3), 2 << 20);
        assert_eq!(
            lines[0],
            "Loaded in 5.0 ms (snapshot + 3 WAL commits replayed): 12,000 vertices, 3 edges"
        );
        assert!(lines[1].starts_with("Vertex labels (12): l0 1,000, l1 999,"));
        assert!(lines[1].ends_with("l9 991, +2 more"));
        assert_eq!(lines[2], "Edge labels (1): knows 3");
        assert_eq!(
            lines[3],
            "Memory: graph 3.5 KiB (structure 1.0 KiB, indexes 512 B, properties 2.0 KiB); \
             process heap 2.0 MiB"
        );
        assert_eq!(
            lines[4],
            "Compressed: 2 values, 4.0 KiB stored as 1.0 KiB (dictionaries 0 B)"
        );
    }

    #[test]
    fn the_memory_line_names_property_indexes_when_there_are_some() {
        let statistics = json!({
            "statistics": {"vertex_count": 1, "edge_count": 0, "vertex_labels": [],
                           "edge_labels": [], "vertex_label_total": 0, "edge_label_total": 0},
            "memory": {
                "structure_bytes": 1024, "index_bytes": 512, "property_bytes": 2048,
                "total_bytes": 5632, "property_index_bytes": 2048,
            },
        });
        let lines = lines(&statistics, Duration::from_millis(5), None, 0);
        assert_eq!(
            lines[1],
            "Memory: graph 5.5 KiB (structure 1.0 KiB, indexes 512 B, properties 2.0 KiB, \
             property indexes 2.0 KiB)"
        );
    }

    #[test]
    fn an_empty_graph_has_no_label_lines() {
        let statistics = json!({
            "statistics": {"vertex_count": 0, "edge_count": 0, "vertex_labels": [],
                           "edge_labels": [], "vertex_label_total": 0, "edge_label_total": 0},
            "memory": null,
        });
        let lines = lines(&statistics, Duration::from_micros(10), None, 0);
        assert_eq!(
            lines,
            vec!["Loaded in 0.01 ms: 0 vertices, 0 edges".to_string()]
        );
    }
}
