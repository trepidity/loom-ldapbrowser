use nucleo::{
    pattern::{CaseMatching, Normalization, Pattern},
    Config, Matcher, Utf32Str,
};

/// Result of fuzzy matching: the original item index and its score.
#[derive(Debug, Clone)]
pub struct FuzzyMatch {
    pub index: usize,
    pub score: u32,
}

/// A fuzzy matcher backed by nucleo.
pub struct FuzzyFilter {
    matcher: Matcher,
}

impl FuzzyFilter {
    pub fn new() -> Self {
        Self {
            matcher: Matcher::new(Config::DEFAULT),
        }
    }

    /// Filter a list of strings by a query pattern.
    /// Returns indices and scores, sorted by score (best first).
    pub fn filter(&mut self, query: &str, items: &[String]) -> Vec<FuzzyMatch> {
        if query.is_empty() {
            return items
                .iter()
                .enumerate()
                .map(|(i, _)| FuzzyMatch { index: i, score: 0 })
                .collect();
        }

        let pattern = Pattern::new(
            query,
            CaseMatching::Ignore,
            Normalization::Smart,
            nucleo::pattern::AtomKind::Fuzzy,
        );

        let mut buf = Vec::new();
        let mut matches: Vec<FuzzyMatch> = items
            .iter()
            .enumerate()
            .filter_map(|(i, item)| {
                let haystack = Utf32Str::new(item, &mut buf);
                let score = pattern.score(haystack, &mut self.matcher)?;
                Some(FuzzyMatch { index: i, score })
            })
            .collect();

        matches.sort_by(|a, b| b.score.cmp(&a.score));
        matches
    }
}

impl Default for FuzzyFilter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzzy_filter_empty_query() {
        let mut filter = FuzzyFilter::new();
        let items = vec!["foo".to_string(), "bar".to_string()];
        let matches = filter.filter("", &items);
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn test_fuzzy_filter_matches() {
        let mut filter = FuzzyFilter::new();
        let items = vec![
            "cn=Alice".to_string(),
            "cn=Bob".to_string(),
            "ou=Users".to_string(),
        ];
        let matches = filter.filter("alice", &items);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].index, 0);
    }
}
