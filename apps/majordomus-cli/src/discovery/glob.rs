//! The `:(glob)` pathspec subset the repository uses: `*` and `?` never cross `/`, `**`
//! matches any number of path segments. Matching is on repository-relative paths.

#[derive(Debug, Clone, PartialEq, Eq)]
enum Segment {
    Any,
    Pattern(String),
}

#[derive(Debug, Clone)]
/// A compiled `:(glob)` pattern.
///
/// ```
/// use majordomus_cli::discovery::glob::Glob;
/// let g = Glob::new(".ai/repo/rules/**/*.md");
/// assert!(g.matches(".ai/repo/rules/project/a.v1.md"));
/// assert!(!g.matches("docs/a.md"));
/// ```
pub struct Glob {
    segments: Vec<Segment>,
}

impl Glob {
    /// Compile a pattern; `**` is a segment of its own, `*` and `?` stay inside one segment.
    pub fn new(pattern: &str) -> Self {
        let segments = pattern
            .split('/')
            .filter(|s| !s.is_empty())
            .map(|s| {
                if s == "**" {
                    Segment::Any
                } else {
                    Segment::Pattern(s.to_string())
                }
            })
            .collect();
        Glob { segments }
    }

    /// Does `path` match the whole pattern?
    pub fn matches(&self, path: &str) -> bool {
        let parts: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        Self::match_from(&self.segments, &parts)
    }

    /// Could some file under directory `dir` match? Used to prune a walk.
    pub fn could_match_under(&self, dir: &str) -> bool {
        let parts: Vec<&str> = dir.split('/').filter(|s| !s.is_empty()).collect();
        Self::prefix_from(&self.segments, &parts)
    }

    fn match_from(segs: &[Segment], parts: &[&str]) -> bool {
        match (segs.first(), parts.first()) {
            (None, None) => true,
            (None, Some(_)) => false,
            (Some(Segment::Any), _) => {
                (0..=parts.len()).any(|skip| Self::match_from(&segs[1..], &parts[skip..]))
            }
            (Some(Segment::Pattern(_)), None) => false,
            (Some(Segment::Pattern(p)), Some(part)) => {
                segment_matches(p, part) && Self::match_from(&segs[1..], &parts[1..])
            }
        }
    }

    fn prefix_from(segs: &[Segment], parts: &[&str]) -> bool {
        match (segs.first(), parts.first()) {
            (_, None) => true,
            (None, Some(_)) => false,
            (Some(Segment::Any), _) => true,
            (Some(Segment::Pattern(p)), Some(part)) => {
                segment_matches(p, part) && Self::prefix_from(&segs[1..], &parts[1..])
            }
        }
    }
}

/// One segment against one path component; `*` and `?` stay inside the component.
fn segment_matches(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (mut pi, mut ti) = (0, 0);
    let (mut star, mut mark) = (None, 0);
    while ti < t.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ti;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ti = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn star_does_not_cross_separators() {
        let g = Glob::new("docs/*.md");
        assert!(g.matches("docs/CLI.md"));
        assert!(!g.matches("docs/claims/x.md"));
        assert!(!g.matches("CLI.md"));
    }

    #[test]
    fn double_star_crosses_any_depth() {
        let g = Glob::new(".ai/repo/rules/**/*.md");
        assert!(g.matches(".ai/repo/rules/project/a.v1.md"));
        assert!(g.matches(".ai/repo/rules/vendor/majordomus/rules/b.v1.md"));
        assert!(g.matches(".ai/repo/rules/README.md"));
        assert!(!g.matches(".ai/repo/prompts/a.md"));
    }

    #[test]
    fn root_star_matches_only_root_files() {
        let g = Glob::new("*.md");
        assert!(g.matches("README.md"));
        assert!(!g.matches("docs/README.md"));
    }

    #[test]
    fn exact_path_matches_itself_only() {
        let g = Glob::new(".ai/repo/policy.yaml");
        assert!(g.matches(".ai/repo/policy.yaml"));
        assert!(!g.matches(".ai/repo/policy.yaml.bak"));
    }

    #[test]
    fn prefix_pruning() {
        let g = Glob::new(".ai/repo/rules/**/*.md");
        assert!(g.could_match_under(".ai"));
        assert!(g.could_match_under(".ai/repo/rules/vendor"));
        assert!(!g.could_match_under("docs"));
        assert!(Glob::new("*.md").could_match_under(""));
        assert!(!Glob::new("*.md").could_match_under("docs"));
    }

    #[test]
    fn question_mark_and_mixed() {
        assert!(segment_matches("I0?0?.yaml", "I0101.yaml"));
        assert!(segment_matches("*.v1.md", "a.v1.md"));
        assert!(!segment_matches("*.v1.md", "a.v2.md"));
    }
}
