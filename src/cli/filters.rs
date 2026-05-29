use std::collections::HashMap;

/// Parse Docker CLI filters (e.g., "status=running") into HashMap for Bollard
///
/// # Arguments
/// * `filter_args` - Slice of filter strings in "key=value" format
///
/// # Returns
/// * `Ok(HashMap)` - Parsed filters ready for Bollard API
/// * `Err(String)` - Error message for invalid filter format
///
/// # Examples
/// ```
/// let filters = vec!["status=running".to_string(), "name=nginx".to_string()];
/// let parsed = parse_filters(&filters).unwrap();
/// assert_eq!(parsed.get("status"), Some(&vec!["running".to_string()]));
/// ```
pub fn parse_filters(filter_args: &[String]) -> Result<HashMap<String, Vec<String>>, String> {
    let mut filters: HashMap<String, Vec<String>> = HashMap::new();

    for filter in filter_args {
        let Some((key, value)) = filter.split_once('=') else {
            return Err(format!(
                "Invalid filter format: '{filter}'. Expected 'key=value'"
            ));
        };

        filters
            .entry(key.to_string())
            .or_default()
            .push(value.to_string());
    }

    Ok(filters)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_single_filter() {
        let filters = vec!["status=running".to_string()];
        let parsed = parse_filters(&filters).unwrap();
        assert_eq!(parsed.get("status"), Some(&vec!["running".to_string()]));
    }

    #[test]
    fn test_parse_multiple_filters() {
        let filters = vec!["status=running".to_string(), "name=nginx".to_string()];
        let parsed = parse_filters(&filters).unwrap();
        assert_eq!(parsed.get("status"), Some(&vec!["running".to_string()]));
        assert_eq!(parsed.get("name"), Some(&vec!["nginx".to_string()]));
    }

    #[test]
    fn test_parse_multiple_values_same_key() {
        let filters = vec!["status=running".to_string(), "status=paused".to_string()];
        let parsed = parse_filters(&filters).unwrap();
        assert_eq!(
            parsed.get("status"),
            Some(&vec!["running".to_string(), "paused".to_string()])
        );
    }

    #[test]
    fn test_parse_label_filter() {
        let filters = vec!["label=com.example.version=1.0".to_string()];
        let parsed = parse_filters(&filters).unwrap();
        assert_eq!(
            parsed.get("label"),
            Some(&vec!["com.example.version=1.0".to_string()])
        );
    }

    #[test]
    fn test_parse_filter_with_special_characters() {
        let filters = vec!["label=environment=production".to_string()];
        let parsed = parse_filters(&filters).unwrap();
        assert_eq!(
            parsed.get("label"),
            Some(&vec!["environment=production".to_string()])
        );
    }

    #[test]
    fn test_parse_empty_value() {
        let filters = vec!["label=".to_string()];
        let parsed = parse_filters(&filters).unwrap();
        assert_eq!(parsed.get("label"), Some(&vec!["".to_string()]));
    }

    #[test]
    fn test_parse_invalid_format_no_equals() {
        let filters = vec!["status".to_string()];
        let result = parse_filters(&filters);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid filter format"));
    }

    #[test]
    fn test_parse_empty_filter_list() {
        let filters: Vec<String> = vec![];
        let parsed = parse_filters(&filters).unwrap();
        assert!(parsed.is_empty());
    }

    #[test]
    fn test_parse_ancestor_filter() {
        let filters = vec!["ancestor=ubuntu:24.04".to_string()];
        let parsed = parse_filters(&filters).unwrap();
        assert_eq!(
            parsed.get("ancestor"),
            Some(&vec!["ubuntu:24.04".to_string()])
        );
    }

    #[test]
    fn test_parse_network_filter() {
        let filters = vec!["network=bridge".to_string()];
        let parsed = parse_filters(&filters).unwrap();
        assert_eq!(parsed.get("network"), Some(&vec!["bridge".to_string()]));
    }
}
