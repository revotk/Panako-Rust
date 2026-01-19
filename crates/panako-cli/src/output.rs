//! JSON output formatting

use panako_core::matching::QueryResult;
use serde::Serialize;

#[derive(Serialize)]
struct MatchOutput {
    query_path: String,
    detections: usize,
    results: Vec<QueryResult>,
}

/// Print query result as JSON
pub fn print_json_result(result: &QueryResult) {
    match serde_json::to_string_pretty(result) {
        Ok(json) => println!("{}", json),
        Err(e) => eprintln!("Error serializing result: {}", e),
    }
}

/// Print multiple results as JSON array with detection count
pub fn print_json_results(results: &[QueryResult]) {
    // Minimum duration threshold in seconds
    const MIN_DURATION_SECONDS: f64 = 2.0;
    
    // Filter out results with no reference and duration < 2 seconds
    let mut valid_results: Vec<_> = results
        .iter()
        .filter(|r| {
            // Filter out results with no reference identifier
            if r.ref_identifier.is_none() {
                log::debug!("Filtered match: no reference identifier");
                return false;
            }
            
            // Filter out detections with duration < 2 seconds
            let duration = r.query_stop - r.query_start;
            if duration < MIN_DURATION_SECONDS {
                log::debug!(
                    "Filtered match: duration {:.2}s < {:.2}s (ref: {:?})",
                    duration,
                    MIN_DURATION_SECONDS,
                    r.ref_identifier
                );
                return false;
            }
            
            true
        })
        .cloned()
        .collect();
    
    // Sort by query_start time (chronological order)
    valid_results.sort_by(|a, b| {
        a.query_start.partial_cmp(&b.query_start).unwrap_or(std::cmp::Ordering::Equal)
    });
    
    if valid_results.len() < results.len() {
        log::info!(
            "Filtered {} matches without reference ({} valid matches remain)",
            results.len() - valid_results.len(),
            valid_results.len()
        );
    }
    
    // Extract query path from first result, or use empty string
    let query_path = valid_results
        .first()
        .map(|r| r.query_path.clone())
        .unwrap_or_else(|| results.first().map(|r| r.query_path.clone()).unwrap_or_default());
    
    let output = MatchOutput {
        query_path,
        detections: valid_results.len(),
        results: valid_results,
    };
    
    match serde_json::to_string_pretty(&output) {
        Ok(json) => println!("{}", json),
        Err(e) => eprintln!("Error serializing results: {}", e),
    }
}
