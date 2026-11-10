//! Parser for user-typed page ranges such as "1-3, 5".

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RangeError {
    InvalidFormat { item: String },
    ZeroPage,
    OutOfRange { page: usize, total: usize },
    Reversed { start: usize, end: usize },
}

/// Returns one group per comma-separated item, as 0-based grid positions.
/// Empty or whitespace-only input returns Ok(vec![]).
pub fn parse(input: &str, total: usize) -> Result<Vec<Vec<usize>>, RangeError> {
    let mut groups = Vec::new();
    for item in input.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let bad = || RangeError::InvalidFormat { item: item.into() };
        let mut ends = item.split('-').map(|n| n.trim().parse::<usize>());
        let start = ends.next().and_then(Result::ok).ok_or_else(bad)?;
        let end = match ends.next() {
            None => start,
            Some(n) => n.map_err(|_| bad())?,
        };
        if ends.next().is_some() {
            return Err(bad());
        }
        for page in [start, end] {
            if page == 0 {
                return Err(RangeError::ZeroPage);
            }
            if page > total {
                return Err(RangeError::OutOfRange { page, total });
            }
        }
        if start > end {
            return Err(RangeError::Reversed { start, end });
        }
        groups.push((start - 1..end).collect());
    }
    Ok(groups)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_groups() {
        assert_eq!(parse("1-3, 5", 10), Ok(vec![vec![0, 1, 2], vec![4]]));
        assert_eq!(parse(" 1 - 3 ", 10), Ok(vec![vec![0, 1, 2]]));
        assert_eq!(parse("  7 ", 10), Ok(vec![vec![6]]));
    }

    #[test]
    fn ignores_whitespace_and_empty_items() {
        assert_eq!(parse("1,,2,", 10), Ok(vec![vec![0], vec![1]]));
    }

    #[test]
    fn allows_duplicates() {
        assert_eq!(parse("1, 1", 10), Ok(vec![vec![0], vec![0]]));
    }

    #[test]
    fn empty_input() {
        assert_eq!(parse("", 10), Ok(vec![]));
    }

    #[test]
    fn rejects_reversed() {
        assert_eq!(
            parse("3-1", 10),
            Err(RangeError::Reversed { start: 3, end: 1 })
        );
    }

    #[test]
    fn rejects_zero() {
        assert_eq!(parse("0", 10), Err(RangeError::ZeroPage));
    }

    #[test]
    fn rejects_out_of_range() {
        let e = |page, total| Err(RangeError::OutOfRange { page, total });
        assert_eq!(parse("12", 10), e(12, 10));
        assert_eq!(parse("5-12", 10), e(12, 10));
        assert_eq!(parse("1", 0), e(1, 0));
    }

    #[test]
    fn rejects_bad_format() {
        let e = |s: &str| Err(RangeError::InvalidFormat { item: s.into() });
        assert_eq!(parse("a", 10), e("a"));
        assert_eq!(parse("1-2-3", 10), e("1-2-3"));
        assert_eq!(parse("1-", 10), e("1-"));
    }
}
