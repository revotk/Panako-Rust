//! Matching algorithm and query results
//!
//! Implements the Panako matching algorithm with JSON output support.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[cfg(test)]
mod tests;

/// Query result matching Java QueryResult structure
/// Output format: JSON for easy parsing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    /// Path of the query file
    pub query_path: String,
    /// Start of match in query (seconds)
    pub query_start: f64,
    /// End of match in query (seconds)
    pub query_stop: f64,
    
    /// Path of the reference file
    pub ref_path: Option<String>,
    /// Internal identifier of the reference
    pub ref_identifier: Option<String>,
    /// Start of match in reference (seconds)
    pub ref_start: f64,
    /// End of match in reference (seconds)
    pub ref_stop: f64,
    
    /// Match score (number of matching fingerprints)
    pub score: i32,
    /// Time factor (percentage)
    pub time_factor: f64,
    /// Frequency factor (percentage)
    pub frequency_factor: f64,
    /// Percentage of seconds with matches
    pub percent_seconds_with_match: f64,
    
    // NEW: Reference duration and absolute positions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ref_duration_ms: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub absolute_start: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub absolute_end: Option<f64>,
}

impl QueryResult {
    /// Create empty result (no match found)
    pub fn empty(query_path: String, query_start: f64, query_stop: f64) -> Self {
        Self {
            query_path,
            query_start,
            query_stop,
            ref_path: None,
            ref_identifier: None,
            ref_start: -1.0,
            ref_stop: -1.0,
            score: -1,
            time_factor: -1.0,
            frequency_factor: -1.0,
            percent_seconds_with_match: 0.0,
            ref_duration_ms: None,
            absolute_start: None,
            absolute_end: None,
        }
    }
}

/// Match between query and reference
#[derive(Debug, Clone)]
struct Match {
    identifier: String,
    query_time: i32,
    match_time: i32,
    query_f1: i16,
    match_f1: i16,
}

impl Match {
    fn delta_t(&self) -> i32 {
        self.match_time - self.query_time
    }
}

/// Matcher for finding/// Matcher for fingerprints
pub struct Matcher {
    /// Inverted index: hash -> Vec<(identifier, t1, f1)>
    index: HashMap<u64, Vec<(String, i32, i16)>>,
    /// Reference durations: identifier -> duration_ms
    ref_durations: HashMap<String, u32>,
}

impl Matcher {
    pub fn new() -> Self {
        Self {
            index: HashMap::new(),
            ref_durations: HashMap::new(),
        }
    }

    /// Add fingerprints to the index
    pub fn add_fingerprints(&mut self, identifier: String, fingerprints: &[(u64, i32, i16, f32)]) {
        for (hash, t1, f1, _m1) in fingerprints {
            self.index
                .entry(*hash)
                .or_insert_with(Vec::new)
                .push((identifier.clone(), *t1, *f1));
        }
    }
    
    /// Add reference duration
    pub fn add_duration(&mut self, identifier: String, duration_ms: u32) {
        self.ref_durations.insert(identifier, duration_ms);
    }
    
    /// Query the index with fingerprints
    pub fn query(
        &self,
        query_path: &str,
        query_fingerprints: &[(u64, i32, i16, f32)],
    ) -> Result<Vec<QueryResult>> {
        // Find all matches
        let mut matches: Vec<Match> = Vec::new();

        // Find matches
        for (hash, t1, f1, _m1) in query_fingerprints {
            if let Some(candidates) = self.index.get(hash) {
                for (identifier, ref_t1, ref_f1) in candidates {
                    matches.push(Match {
                        identifier: identifier.clone(),
                        query_time: *t1,
                        match_time: *ref_t1,
                        query_f1: *f1,
                        match_f1: *ref_f1,
                    });
                }
            }
        }
        
        if matches.is_empty() {
            return Ok(vec![]);  // Return empty array instead of empty result
        }
        
        // Group by identifier and find most common delta_t
        let mut results = Vec::new();
        let mut by_identifier: HashMap<String, Vec<Match>> = HashMap::new();
        
        for m in matches {
            by_identifier
                .entry(m.identifier.clone())
                .or_insert_with(Vec::new)
                .push(m);
        }
        
        // Early filtering: skip identifiers with very few matches
        const MIN_MATCH_THRESHOLD: usize = 5;
        const MIN_ALIGNED_THRESHOLD: usize = 5;
        
        for (identifier, id_matches) in by_identifier {
            if id_matches.len() < MIN_MATCH_THRESHOLD {
                log::trace!(
                    "Skipping {}: only {} raw matches (need {})",
                    identifier,
                    id_matches.len(),
                    MIN_MATCH_THRESHOLD
                );
                continue;
            }
            
            // Find most common delta_t (time offset)
            let mut delta_histogram: HashMap<i32, usize> = HashMap::new();
            for m in &id_matches {
                *delta_histogram.entry(m.delta_t()).or_insert(0) += 1;
            }
            
            let (best_delta, count) = delta_histogram
                .iter()
                .max_by_key(|(_, &count)| count)
                .map(|(&delta, &count)| (delta, count))
                .unwrap_or((0, 0));
            
            // Skip if best delta doesn't have enough support
            if count < MIN_ALIGNED_THRESHOLD {
                log::trace!(
                    "Skipping {}: best delta only has {} matches (need {})",
                    identifier,
                    count,
                    MIN_ALIGNED_THRESHOLD
                );
                continue;
            }
            
            // Filter matches by best delta_t (allow ±2 frame tolerance)
            let aligned_matches: Vec<_> = id_matches
                .iter()
                .filter(|m| (m.delta_t() - best_delta).abs() <= 2)
                .collect();
            
            if aligned_matches.len() < MIN_ALIGNED_THRESHOLD {
                continue;
            }
            
            log::debug!(
                "Identifier: {}, raw matches: {}, aligned: {}, best_delta: {}",
                identifier,
                id_matches.len(),
                aligned_matches.len(),
                best_delta
            );
            
            // Estimate time bounds
            let query_times: Vec<i32> = aligned_matches.iter().map(|m| m.query_time).collect();
            let match_times: Vec<i32> = aligned_matches.iter().map(|m| m.match_time).collect();
            
            let query_start_frame = *query_times.iter().min().unwrap();
            let query_stop_frame = *query_times.iter().max().unwrap();
            let query_start = query_start_frame as f64 * 0.008; // ~8ms per frame
            let query_stop = query_stop_frame as f64 * 0.008;
            let ref_start = *match_times.iter().min().unwrap() as f64 * 0.008;
            let ref_stop = *match_times.iter().max().unwrap() as f64 * 0.008;
            
            // Calculate factors using helper functions
            let time_factor = calculate_time_factor(&aligned_matches);
            let frequency_factor = calculate_frequency_factor(&aligned_matches);
            let coverage = calculate_coverage(&aligned_matches, query_start_frame, query_stop_frame);

            log::debug!(
                "Identifier: {}, raw matches: {}, aligned: {}, best_delta: {}",
                identifier,
                id_matches.len(), // Corrected from raw_matches.len()
                aligned_matches.len(),
                best_delta
            );
            log::debug!(
                "  time_factor: {:.3}, freq_factor: {:.3}, coverage: {:.1}%",
                time_factor,
                frequency_factor,
                coverage * 100.0
            );

            // Get reference duration if available
            let ref_duration_ms = self.ref_durations.get(&identifier).copied();
            
            // Calculate absolute positions
            let (absolute_start, absolute_end) = if let Some(duration_ms) = ref_duration_ms {
                let abs_start = query_start - ref_start;
                let abs_end = abs_start + (duration_ms as f64 / 1000.0);
                (Some(abs_start), Some(abs_end))
            } else {
                (None, None)
            };

            results.push(QueryResult {
                query_path: query_path.to_string(),
                query_start,
                query_stop,
                ref_path: Some(identifier.clone()), // Kept as Some()
                ref_identifier: Some(identifier.clone()),
                ref_start,
                ref_stop,
                score: aligned_matches.len() as i32,
                time_factor,
                frequency_factor,
                percent_seconds_with_match: coverage,
                ref_duration_ms,
                absolute_start,
                absolute_end,
            });
        }
        
        // Sort by score descending
        results.sort_by(|a, b| b.score.cmp(&a.score));
        
        // Return all results (no max_results limit)
        Ok(results)
    }
}

impl Default for Matcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Calculate time factor (speed ratio) using linear regression
/// Returns the slope of query_time vs match_time
/// 1.0 = normal speed, > 1.0 = sped up, < 1.0 = slowed down
fn calculate_time_factor(matches: &[&Match]) -> f64 {
    if matches.len() < 2 {
        return 1.0;
    }
    
    // Linear regression: match_time = slope * query_time + intercept
    let n = matches.len() as f64;
    let sum_x: f64 = matches.iter().map(|m| m.query_time as f64).sum();
    let sum_y: f64 = matches.iter().map(|m| m.match_time as f64).sum();
    let sum_xy: f64 = matches.iter().map(|m| m.query_time as f64 * m.match_time as f64).sum();
    let sum_x2: f64 = matches.iter().map(|m| (m.query_time as f64).powi(2)).sum();
    
    let denominator = n * sum_x2 - sum_x * sum_x;
    if denominator.abs() < 1e-10 {
        return 1.0;
    }
    
    let slope = (n * sum_xy - sum_x * sum_y) / denominator;
    
    // Clamp to reasonable range (0.5x to 2.0x speed)
    slope.max(0.5).min(2.0)
}

/// Calculate frequency factor (pitch ratio)
/// Returns the average ratio of matched frequencies
/// 1.0 = no pitch change, > 1.0 = higher pitch, < 1.0 = lower pitch
fn calculate_frequency_factor(matches: &[&Match]) -> f64 {
    if matches.is_empty() {
        return 1.0;
    }
    
    let mut ratios = Vec::new();
    for m in matches {
        if m.query_f1 > 0 && m.match_f1 > 0 {
            let ratio = m.match_f1 as f64 / m.query_f1 as f64;
            // Only include reasonable ratios (within 2 octaves)
            if ratio >= 0.25 && ratio <= 4.0 {
                ratios.push(ratio);
            }
        }
    }
    
    if ratios.is_empty() {
        return 1.0;
    }
    
    // Use median instead of mean to be robust to outliers
    ratios.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = if ratios.len() % 2 == 0 {
        (ratios[ratios.len() / 2 - 1] + ratios[ratios.len() / 2]) / 2.0
    } else {
        ratios[ratios.len() / 2]
    };
    
    median
}

/// Calculate percentage of query seconds that have matches
/// Returns value between 0.0 and 1.0
fn calculate_coverage(matches: &[&Match], query_start: i32, query_stop: i32) -> f64 {
    if matches.is_empty() || query_stop <= query_start {
        return 0.0;
    }
    
    // Count unique seconds that have matches
    let mut covered_seconds = std::collections::HashSet::new();
    for m in matches {
        // Convert frame index to seconds (assuming ~8ms per frame)
        let second = (m.query_time as f64 * 0.008).floor() as i32;
        covered_seconds.insert(second);
    }
    
    let total_seconds = ((query_stop - query_start) as f64 * 0.008).ceil() as i32;
    if total_seconds <= 0 {
        return 0.0;
    }
    
    covered_seconds.len() as f64 / total_seconds as f64
}
