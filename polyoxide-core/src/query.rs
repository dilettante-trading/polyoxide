//! Comma-joined query values.

/// Comma-joins a multi-value query parameter; `None` when the joined value is
/// empty, so the parameter is omitted rather than sent empty.
///
/// ```
/// assert_eq!(polyoxide_core::csv(["a", "b"]), Some("a,b".to_owned()));
/// assert_eq!(polyoxide_core::csv(Vec::<String>::new()), None);
/// assert_eq!(polyoxide_core::csv([""]), None);
/// ```
pub fn csv<I, S>(values: I) -> Option<String>
where
    I: IntoIterator<Item = S>,
    S: ToString,
{
    let joined = values
        .into_iter()
        .map(|v| v.to_string())
        .collect::<Vec<String>>()
        .join(",");
    (!joined.is_empty()).then_some(joined)
}

#[cfg(test)]
mod tests {
    use super::csv;

    #[test]
    fn csv_joins_values_with_commas() {
        assert_eq!(csv(["0xa", "0xb"]), Some("0xa,0xb".to_owned()));
        assert_eq!(csv([7, 8]), Some("7,8".to_owned()));
    }

    #[test]
    fn csv_of_nothing_is_none_so_the_parameter_is_omitted() {
        assert_eq!(csv(Vec::<String>::new()), None);
    }
}
