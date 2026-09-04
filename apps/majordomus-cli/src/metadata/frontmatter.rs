//! Front matter: a YAML block between a first line `---` and the next line `---`, as the
//! shell tool's `mj_record_front` reads it. Nothing else is recognised: no `+++`, no JSON,
//! no leading blank line.

use serde_json::{Map, Value};

use super::yaml;

/// Front matter larger than this is refused rather than parsed.
pub const MAX_FRONT_MATTER_BYTES: usize = 64 * 1024;

/// A document split into its parts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Split<'a> {
    /// The raw YAML between the fences, without them.
    pub front: Option<&'a str>,
    /// Everything after the closing fence, or the whole text when there is none.
    pub body: &'a str,
}

/// Why a document could not be split.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SplitError {
    #[error("front matter opened on line 1 and never closed")]
    Unclosed,
    #[error("front matter is {0} bytes, over the {MAX_FRONT_MATTER_BYTES} byte limit")]
    Oversized(usize),
}

/// Split a document. Text that does not open with `---` has no front matter and is all body.
pub fn split(text: &str) -> Result<Split<'_>, SplitError> {
    let Some(rest) = text
        .strip_prefix("---\n")
        .or_else(|| text.strip_prefix("---\r\n"))
    else {
        if text == "---" {
            return Err(SplitError::Unclosed);
        }
        return Ok(Split {
            front: None,
            body: text,
        });
    };
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\n', '\r']);
        if trimmed == "---" {
            let front = &rest[..offset];
            if front.len() > MAX_FRONT_MATTER_BYTES {
                return Err(SplitError::Oversized(front.len()));
            }
            return Ok(Split {
                front: Some(front),
                body: &rest[offset + line.len()..],
            });
        }
        offset += line.len();
    }
    Err(SplitError::Unclosed)
}

/// Parse the front matter of a split document into a mapping.
pub fn parse(front: &str) -> Result<Map<String, Value>, String> {
    yaml::parse_mapping(front)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_a_rule_file() {
        let s = split("---\nid: x\nversion: 1\n---\n\n# Body\n").unwrap();
        assert_eq!(s.front, Some("id: x\nversion: 1\n"));
        assert_eq!(s.body, "\n# Body\n");
        assert_eq!(parse(s.front.unwrap()).unwrap()["version"], 1);
    }

    #[test]
    fn no_fence_means_no_front_matter() {
        let s = split("# Title\n---\nnot front matter\n").unwrap();
        assert_eq!(s.front, None);
    }

    #[test]
    fn unclosed_is_an_error_not_a_body() {
        assert_eq!(split("---\nid: x\n"), Err(SplitError::Unclosed));
        assert_eq!(split("---"), Err(SplitError::Unclosed));
    }

    #[test]
    fn oversized_is_refused() {
        let big = format!("---\nk: {}\n---\n", "a".repeat(MAX_FRONT_MATTER_BYTES));
        assert!(matches!(split(&big), Err(SplitError::Oversized(_))));
    }

    #[test]
    fn empty_front_matter_is_an_empty_mapping() {
        let s = split("---\n---\nbody").unwrap();
        assert_eq!(s.front, Some(""));
        assert!(parse("").unwrap().is_empty());
    }
}
