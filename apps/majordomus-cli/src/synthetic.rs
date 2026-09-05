//! A synthetic repository: the layer `majordomus init` writes, with as many rules and
//! documents as a test or a benchmark asks for, in a fresh temporary directory that is
//! removed on drop. One generator for the property tests, the scaling benchmarks and the
//! registry benchmarks, so that they measure and test the same shape of repository. Not
//! a git repository: discovery goes through the filesystem, which is what a benchmark
//! wants to measure and a property test wants to control.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::capability::{builtin, CapabilityRegistry, Context};
use crate::discovery::{FileSystem, Sources};
use crate::git::GitState;
use crate::index::Index;
use crate::metadata::KindSchema;
use crate::repository::Repository;
use crate::share::Share;

const MANIFEST: &str = "schema: ai-repository/v1\nrepo:\n  path: repo\nlocal:\n  path: local\n  tracked: false\n  implicit_context: false\nsections:\n  policy: repo/policy.yaml\n  profiles: repo/profiles\n  rules: repo/rules\n  prompts: repo/prompts\n  skills: repo/skills\n  workflows: repo/workflows\n  knowledge: repo/knowledge\n  adrs: repo/adrs\n  project: repo/project\n";
const SOURCES: &str = "version: 1\nsources:\n  - id: policy\n    kind: policy\n    discovery: vcs\n    pathspec: ':(glob).ai/repo/policy.yaml'\n    required: true\n  - id: profile\n    kind: profile\n    discovery: vcs\n    pathspec: ':(glob).ai/repo/profiles/*.yaml'\n    required: true\n  - id: rule\n    kind: rule\n    discovery: vcs\n    pathspec: ':(glob).ai/repo/rules/**/*.md'\n    required: true\n  - id: prompt\n    kind: prompt\n    discovery: vcs\n    pathspec: ':(glob).ai/repo/prompts/*.md'\n    required: false\n  - id: document\n    kind: document\n    discovery: vcs\n    pathspec: ':(glob)docs/*.md'\n    required: false\n";

static SEQ: AtomicU64 = AtomicU64::new(0);

/// How large the repository is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shape {
    /// Project rules under `.ai/repo/rules/project/`.
    pub rules: usize,
    /// Prompts under `.ai/repo/prompts/`.
    pub prompts: usize,
    /// Documents under `docs/`.
    pub documents: usize,
    /// Lines of body per rule and document.
    pub body_lines: usize,
}

impl Default for Shape {
    fn default() -> Self {
        Shape {
            rules: 10,
            prompts: 2,
            documents: 5,
            body_lines: 8,
        }
    }
}

/// A generated repository on disk, removed when dropped.
#[derive(Debug)]
pub struct SyntheticRepository {
    root: PathBuf,
    /// The shape it was built with.
    pub shape: Shape,
}

impl SyntheticRepository {
    /// Generate a repository of this shape under the system's temporary directory.
    pub fn new(shape: Shape) -> std::io::Result<Self> {
        let seq = SEQ.fetch_add(1, Ordering::SeqCst);
        let root =
            std::env::temp_dir().join(format!("majordomus-synthetic-{}-{seq}", std::process::id()));
        std::fs::create_dir_all(&root)?;
        let repo = SyntheticRepository { root, shape };
        repo.write(".ai/manifest.yaml", MANIFEST)?;
        repo.write(
            ".ai/repo/policy.yaml",
            "version: 1\ncontext:\n  always_loaded_budget_lines: 150\n",
        )?;
        repo.write(".ai/repo/profiles/implementation.yaml", "name: implementation\ndescription: d\ncapability: standard\neffort: medium\nverbosity: concise\npresentation: engineering\ncheckpoint_interval: 15m\n")?;
        repo.write(".ai/repo/knowledge/sources.yaml", SOURCES)?;
        for i in 0..shape.rules {
            repo.write(
                &format!(".ai/repo/rules/project/rule-{i}.v1.md"),
                &rule(i, shape.body_lines),
            )?;
        }
        for i in 0..shape.prompts {
            repo.write(
                &format!(".ai/repo/prompts/prompt-{i}.md"),
                &format!("---\nname: prompt-{i}\ndescription: Prompt number {i}.\n---\n\n# Prompt {i}\n\n{{{{CONTEXT}}}}\n"),
            )?;
        }
        for i in 0..shape.documents {
            let body: String = (0..shape.body_lines)
                .map(|l| {
                    format!("Line {l} of document {i}: the quick brown fox names scope and rule.\n")
                })
                .collect();
            repo.write(
                &format!("docs/DOC_{i}.md"),
                &format!("# Document {i}\n\n{body}"),
            )?;
        }
        Ok(repo)
    }

    /// The default shape.
    pub fn small() -> std::io::Result<Self> {
        Self::new(Shape::default())
    }

    /// The repository root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn write(&self, rel: &str, content: &str) -> std::io::Result<()> {
        let p = self.root.join(rel);
        if let Some(dir) = p.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(p, content)
    }

    /// Change one rule's body: the repository state, and so every fingerprint, moves.
    pub fn touch_rule(&self, i: usize, suffix: &str) -> std::io::Result<()> {
        let rel = format!(".ai/repo/rules/project/rule-{i}.v1.md");
        let text = std::fs::read_to_string(self.root.join(&rel))?;
        self.write(&rel, &format!("{text}\n{suffix}\n"))
    }

    /// The index of this repository, discovered through the filesystem, with the
    /// distribution's share directory beside the crate.
    pub fn index(&self) -> crate::error::Result<Index> {
        let repo = Repository::discover(&self.root)?;
        let sources = Sources::load(&repo)?;
        let share = Share::locate(Some(&crate_share()), &self.root)?;
        let schema = KindSchema::load(&share, &repo)?;
        let fs = FileSystem {
            excluded: vec![".git".into(), repo.local_path()],
        };
        Index::build(
            &repo,
            &sources,
            &schema,
            &fs,
            GitState::Unavailable {
                reason: "synthetic repository".into(),
            },
        )
    }

    /// A context over this repository: index, the application's modules, a fresh executor.
    pub fn context(&self) -> crate::error::Result<std::sync::Arc<Context>> {
        let index = self.index()?;
        let registry = CapabilityRegistry::builder()
            .with_modules(builtin::modules())
            .with_index(&index)
            .build()
            .map_err(|errors| crate::error::Error::Registry { errors })?;
        Ok(std::sync::Arc::new(Context::new(
            std::sync::Arc::new(index),
            std::sync::Arc::new(registry),
        )))
    }
}

impl Drop for SyntheticRepository {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

/// The tool distribution's share directory beside this crate.
pub fn crate_share() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../share")
}

fn rule(i: usize, body_lines: usize) -> String {
    let body: String = (0..body_lines)
        .map(|l| format!("Rationale line {l} of rule {i}: keep the scope and check the state.\n"))
        .collect();
    format!("---\nid: project.rule-{i}\nversion: 1\nkind: rule\ntitle: Rule {i}\ndescription: Rule number {i}, in one sentence.\nstatement: Do the thing number {i}.\nstatus: active\nclass: advisory\ndepends_on: []\ntags: [synthetic]\n---\n\n# Rationale\n\n{body}")
}
