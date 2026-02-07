//! Fuzzy string matching for search/filter operations.
//!
//! This module provides fuzzy matching algorithms with no ratatui dependencies.
//! Used by the new session dialog fuzzy modal for filtering items.

/// Fuzzy match result with score for sorting
#[derive(Debug, Clone)]
struct FuzzyMatch {
    index: usize,
    score: i32,
}

/// Perform fuzzy matching on a list of items
///
/// Returns indices of matching items, sorted by score (best first)
pub fn fuzzy_filter(query: &str, items: &[String]) -> Vec<usize> {
    if query.is_empty() {
        return (0..items.len()).collect();
    }

    let query_lower = query.to_lowercase();
    let query_chars: Vec<char> = query_lower.chars().collect();

    let mut matches: Vec<FuzzyMatch> = items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            fuzzy_score(&query_chars, &item.to_lowercase()).map(|score| FuzzyMatch { index, score })
        })
        .collect();

    // Sort by score (higher is better), then by original index for stability
    matches.sort_by(|a, b| b.score.cmp(&a.score).then(a.index.cmp(&b.index)));

    matches.into_iter().map(|m| m.index).collect()
}

/// Calculate fuzzy match score
///
/// Returns None if no match, Some(score) if matched.
/// Higher score = better match.
///
/// Scoring:
/// - Base points for each matched character
/// - Bonus for consecutive matches
/// - Bonus for matches at word boundaries
/// - Bonus for exact prefix match
fn fuzzy_score(query: &[char], target: &str) -> Option<i32> {
    if query.is_empty() {
        return Some(0);
    }

    let target_chars: Vec<char> = target.chars().collect();
    if target_chars.is_empty() {
        return None;
    }

    let mut score: i32 = 0;
    let mut query_idx = 0;
    let mut prev_match_idx: Option<usize> = None;
    let mut first_match_idx: Option<usize> = None;

    for (target_idx, &target_char) in target_chars.iter().enumerate() {
        if query_idx < query.len() && target_char == query[query_idx] {
            // Found a match
            score += 10; // Base score for match

            // Track first match position
            if first_match_idx.is_none() {
                first_match_idx = Some(target_idx);
            }

            // Bonus for consecutive matches
            if let Some(prev_idx) = prev_match_idx {
                if target_idx == prev_idx + 1 {
                    score += 15; // Consecutive bonus
                }
            }

            // Bonus for word boundary match
            if target_idx == 0 || !target_chars[target_idx - 1].is_alphanumeric() {
                score += 10; // Word start bonus
            }

            // Bonus for uppercase match (camelCase)
            if target_char.is_uppercase() {
                score += 5;
            }

            prev_match_idx = Some(target_idx);
            query_idx += 1;
        }
    }

    // Did we match all query characters?
    if query_idx < query.len() {
        return None;
    }

    // Bonus for prefix match
    if first_match_idx == Some(0) {
        score += 20;
    }

    // Penalty for longer targets (prefer shorter matches)
    score -= (target_chars.len() as i32) / 5;

    Some(score)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzzy_filter_empty_query() {
        let items = vec!["a".into(), "b".into(), "c".into()];
        let result = fuzzy_filter("", &items);
        assert_eq!(result, vec![0, 1, 2]);
    }

    #[test]
    fn test_fuzzy_filter_exact_match() {
        let items = vec!["dev".into(), "staging".into(), "production".into()];
        let result = fuzzy_filter("dev", &items);
        assert!(result.contains(&0)); // "dev" matches
    }

    #[test]
    fn test_fuzzy_filter_partial_match() {
        let items = vec!["development".into(), "staging".into(), "dev".into()];
        let result = fuzzy_filter("dev", &items);

        // Both "development" and "dev" match, but "dev" should rank higher
        assert!(result.len() >= 2);
        // Exact match should be first due to length penalty on "development"
    }

    #[test]
    fn test_fuzzy_filter_fuzzy_match() {
        let items = vec!["devStaging".into(), "staging".into()];
        let result = fuzzy_filter("dS", &items);

        // "dS" should match "devStaging" (d...S)
        assert!(result.contains(&0));
    }

    #[test]
    fn test_fuzzy_filter_case_insensitive() {
        let items = vec!["DevStaging".into(), "PRODUCTION".into()];
        let result = fuzzy_filter("dev", &items);
        assert!(result.contains(&0));

        let result2 = fuzzy_filter("prod", &items);
        assert!(result2.contains(&1));
    }

    #[test]
    fn test_fuzzy_filter_no_match() {
        let items = vec!["alpha".into(), "beta".into()];
        let result = fuzzy_filter("xyz", &items);
        assert!(result.is_empty());
    }

    #[test]
    fn test_fuzzy_score_consecutive_bonus() {
        let query: Vec<char> = "dev".chars().collect();
        let score1 = fuzzy_score(&query, "dev").unwrap();
        let score2 = fuzzy_score(&query, "d_e_v").unwrap();

        // Consecutive should score higher
        assert!(score1 > score2);
    }

    #[test]
    fn test_fuzzy_score_prefix_bonus() {
        let query: Vec<char> = "st".chars().collect();
        let score1 = fuzzy_score(&query, "staging").unwrap();
        let score2 = fuzzy_score(&query, "test").unwrap();

        // Prefix match should score higher
        assert!(score1 > score2);
    }
}
