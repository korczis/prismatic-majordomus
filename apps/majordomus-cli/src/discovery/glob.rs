//! The `:(glob)` pathspec subset the repository's `sources.yaml` is written in.
//!
//! Deliberately small, and the smallness is the point: this has to agree with what git
//! matches for the same pathspec, because discovery asks git for the tracked files and then
//! judges them here. `*` and `?` match within one path segment and never cross a `/`; `**`
//! is a segment of its own and matches any number of segments, none included; and a bracket
//! expression — `[0-9]`, `[abc]`, `[!x]`, `[^x]` — matches one character of its class,
//! inside a segment, as git's own matcher reads it. There is no brace expansion and no
//! negation of a whole pattern: a pattern this cannot express is a pattern the layer does
//! not use.
//!
//! Matching is on repository-relative paths, always with `/`, never with a leading `./`.
//!
//! ```
//! use majordomus_cli::discovery::glob::Glob;
//!
//! let rules = Glob::new(".ai/repo/rules/**/*.md");
//! assert!(rules.matches(".ai/repo/rules/project/scope.v1.md"));
//! assert!(rules.matches(".ai/repo/rules/vendor/majordomus/rules/adr.v1.md"));
//! assert!(!rules.matches(".ai/repo/rules/project/scope.v1.yaml"), "the extension decides");
//!
//! // a single star stays inside its segment, which is what keeps `*.md` from matching a tree
//! let flat = Glob::new(".ai/repo/*.yaml");
//! assert!(flat.matches(".ai/repo/policy.yaml"));
//! assert!(!flat.matches(".ai/repo/ci/gates.yaml"));
//!
//! // a bracket expression matches one character of its class, and negates with ! or ^
//! let versioned = Glob::new(".ai/repo/rules/**/*.v[0-9].md");
//! assert!(versioned.matches(".ai/repo/rules/project/scope.v1.md"));
//! assert!(!versioned.matches(".ai/repo/rules/project/scope.vx.md"));
//!
//! // and a walk can skip a directory the pattern could never match under
//! assert!(rules.could_match_under(".ai/repo/rules"));
//! assert!(!rules.could_match_under("docs"));
//! ```

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

/// One segment against one path component; `*`, `?` and a bracket expression stay inside
/// the component.
fn segment_matches(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    let (mut pi, mut ti) = (0, 0);
    let (mut star, mut mark) = (None, 0);
    while ti < t.len() {
        let class = if pi < p.len() && p[pi] == '[' {
            bracket(&p, pi, t[ti])
        } else {
            None
        };
        if let Some((matched, end)) = class {
            if matched {
                pi = end;
                ti += 1;
                continue;
            } else if let Some(s) = star {
                pi = s + 1;
                mark += 1;
                ti = mark;
                continue;
            } else {
                return false;
            }
        }
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

/// A bracket expression starting at `p[open]` (`[0-9]`, `[abc]`, `[!x]`, `[^x]`): whether
/// `c` is in the class, and the index just past the closing bracket. `None` when the
/// bracket never closes, in which case `[` is an ordinary character, as git treats it.
fn bracket(p: &[char], open: usize, c: char) -> Option<(bool, usize)> {
    let mut i = open + 1;
    let negated = matches!(p.get(i), Some('!') | Some('^'));
    if negated {
        i += 1;
    }
    let mut matched = false;
    let mut first = true;
    while i < p.len() {
        let ch = p[i];
        if ch == ']' && !first {
            return Some((matched != negated, i + 1));
        }
        first = false;
        if i + 2 < p.len() && p[i + 1] == '-' && p[i + 2] != ']' {
            if ch <= c && c <= p[i + 2] {
                matched = true;
            }
            i += 3;
        } else {
            if ch == c {
                matched = true;
            }
            i += 1;
        }
    }
    None
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

    /// The decisions are discovered by `[0-9][0-9][0-9][0-9]-*.md`, and a matcher that
    /// read `[` as a letter discovered none of them while git listed every one.
    #[test]
    fn bracket_classes_match_one_character_as_git_reads_them() {
        let g = Glob::new(".ai/repo/adrs/[0-9][0-9][0-9][0-9]-*.md");
        assert!(g.matches(".ai/repo/adrs/0021-the-topology.md"));
        assert!(!g.matches(".ai/repo/adrs/README.md"));
        assert!(!g.matches(".ai/repo/adrs/021-short.md"));
        assert!(segment_matches("[abc]x", "bx"));
        assert!(!segment_matches("[abc]x", "dx"));
        assert!(segment_matches("[!abc]x", "dx"));
        assert!(segment_matches("[^0-9]?", "az"));
        assert!(segment_matches("a[-x]b", "a-b"));
        // a bracket that never closes is an ordinary character
        assert!(segment_matches("a[b", "a[b"));
        assert!(!segment_matches("a[b", "ab"));
        // classes and stars together, backtracking through the star
        assert!(segment_matches("*[0-9].md", "issue-7.md"));
        assert!(!segment_matches("*[0-9].md", "issue-x.md"));
    }

    #[test]
    fn question_mark_and_mixed() {
        assert!(segment_matches("I0?0?.yaml", "I0101.yaml"));
        assert!(segment_matches("*.v1.md", "a.v1.md"));
        assert!(!segment_matches("*.v1.md", "a.v2.md"));
    }
}
