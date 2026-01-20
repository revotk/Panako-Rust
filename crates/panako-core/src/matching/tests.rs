//! Tests for matching algorithm

use super::*;
use crate::config::PanakoConfig;

#[test]
fn test_matcher_basic() {
    let mut matcher = Matcher::new();
    let config = PanakoConfig::default();
    
    // Create reference fingerprints in tuple format: (hash, t1, f1, m1)
    let ref_fps: Vec<(u64, i32, i16, f32)> = vec![
        (12345, 100, 50, 1.0),
        (67890, 200, 60, 1.0),
        (11111, 300, 70, 1.0),
        (22222, 400, 80, 1.0),
        (33333, 500, 90, 1.0),
        (44444, 600, 100, 1.0),
    ];
    
    matcher.add_fingerprints("test_ref".to_string(), &ref_fps);
    matcher.add_duration("test_ref".to_string(), 5000);
    
    // Create query fingerprints (same as reference)
    let query_fps = ref_fps.clone();
    
    let results = matcher
        .query("test_query", &query_fps, &config)
        .unwrap();
    
    // Should find a match (we have 6 aligned matches)
    assert!(!results.is_empty());
    assert_eq!(results[0].ref_identifier, Some("test_ref".to_string()));
}


#[test]
fn test_time_factor_calculation() {
    // Create matches with normal speed (1:1 ratio)
    let matches: Vec<Match> = vec![
        Match {
            identifier: "test".to_string(),
            query_time: 100,
            match_time: 100,
            query_f1: 50,
            match_f1: 50,
        },
        Match {
            identifier: "test".to_string(),
            query_time: 200,
            match_time: 200,
            query_f1: 50,
            match_f1: 50,
        },
        Match {
            identifier: "test".to_string(),
            query_time: 300,
            match_time: 300,
            query_f1: 50,
            match_f1: 50,
        },
    ];
    
    let match_refs: Vec<&Match> = matches.iter().collect();
    let factor = calculate_time_factor(&match_refs);
    
    // Should be close to 1.0 (normal speed)
    assert!((factor - 1.0).abs() < 0.01);
}

#[test]
fn test_frequency_factor_calculation() {
    // Create matches with same frequency
    let matches: Vec<Match> = vec![
        Match {
            identifier: "test".to_string(),
            query_time: 100,
            match_time: 100,
            query_f1: 50,
            match_f1: 50,
        },
        Match {
            identifier: "test".to_string(),
            query_time: 200,
            match_time: 200,
            query_f1: 60,
            match_f1: 60,
        },
    ];
    
    let match_refs: Vec<&Match> = matches.iter().collect();
    let factor = calculate_frequency_factor(&match_refs);
    
    // Should be close to 1.0 (no pitch change)
    assert!((factor - 1.0).abs() < 0.01);
}

#[test]
fn test_coverage_calculation() {
    // Create matches spanning 3 seconds
    let matches: Vec<Match> = vec![
        Match {
            identifier: "test".to_string(),
            query_time: 0,     // 0 seconds
            match_time: 0,
            query_f1: 50,
            match_f1: 50,
        },
        Match {
            identifier: "test".to_string(),
            query_time: 125,   // ~1 second
            match_time: 125,
            query_f1: 50,
            match_f1: 50,
        },
        Match {
            identifier: "test".to_string(),
            query_time: 250,   // ~2 seconds
            match_time: 250,
            query_f1: 50,
            match_f1: 50,
        },
    ];
    
    let match_refs: Vec<&Match> = matches.iter().collect();
    let coverage = calculate_coverage(&match_refs, 0, 375); // 0-3 seconds
    
    // Should cover all 3 seconds
    assert!(coverage > 0.9); // Allow some rounding
}
