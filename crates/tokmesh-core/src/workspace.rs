use crate::{sessions, UnifiedMessage, UNKNOWN_WORKSPACE_GROUP_KEY, UNKNOWN_WORKSPACE_LABEL};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

/// How workspace rows treat git worktrees.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize)]
pub enum WorktreeRollup {
    /// One row per worktree — a task-isolating agent CLI produces many rows per repo.
    #[default]
    Separate,
    /// Fold every worktree into its parent repository.
    MergeIntoRepo,
}

/// Resolving a workspace key to a display label reads the filesystem (see
/// [`sessions::decode_claude_project_slug`]). Reports iterate hundreds of
/// thousands of messages over a handful of distinct workspaces, so memoize per
/// key and keep the syscalls proportional to workspaces, not messages.
#[derive(Default)]
pub struct WorkspaceLabeler {
    labels: HashMap<String, String>,
    roots: HashMap<String, Option<String>>,
    paths: HashMap<String, Option<String>>,
    decoded: HashMap<String, Option<String>>,
    resolved_roots: HashMap<String, Option<String>>,
}

impl WorkspaceLabeler {
    pub fn label(&mut self, key: &str) -> String {
        if let Some(cached) = self.labels.get(key) {
            return cached.clone();
        }
        let decoded = self.decoded(key);
        let label = sessions::workspace_display_label_for_decoded_key(key, decoded.as_deref())
            .unwrap_or_else(|| UNKNOWN_WORKSPACE_LABEL.to_string());
        self.labels.insert(key.to_string(), label.clone());
        label
    }

    /// The real filesystem path `key` names, decoded from Claude Code's slug
    /// where it is one. `None` when the key is not a path (an opaque client id)
    /// or its directory is gone.
    pub fn path(&mut self, key: &str) -> Option<String> {
        if let Some(cached) = self.paths.get(key) {
            return cached.clone();
        }
        let decoded = self.decoded(key);
        let path = sessions::workspace_path_for_decoded_key(key, decoded.as_deref());
        self.paths.insert(key.to_string(), path.clone());
        path
    }

    /// Claude Code's slug decoded to a real path, memoized.
    ///
    /// The decode is the expensive half of every method here: it walks the
    /// filesystem, backtracking over the ambiguity in the dash encoding. Without
    /// this cache `label`, `path` and `repo_root` each ran their own walk for the
    /// same key, so a slug that took 7s to decode cost 23s across one row.
    fn decoded(&mut self, key: &str) -> Option<String> {
        if let Some(cached) = self.decoded.get(key) {
            return cached.clone();
        }
        let decoded = sessions::decode_claude_project_slug(key);
        self.decoded.insert(key.to_string(), decoded.clone());
        decoded
    }

    /// Distinct keys whose slug decode has been resolved, for tests that need to
    /// prove the walk is shared across `label`, `path` and `repo_root`.
    #[cfg(test)]
    pub(crate) fn decoded_key_count(&self) -> usize {
        self.decoded.len()
    }

    /// The repo root a resolved filesystem path belongs to, memoized.
    ///
    /// Distinct from [`Self::repo_root`], which is keyed by the workspace key and
    /// applies the slug fallbacks. This one is keyed by path because it reads the
    /// `.git` pointer file, and several keys can resolve to the same directory.
    pub fn repo_root_of_path(&mut self, path: &str) -> Option<String> {
        if let Some(cached) = self.resolved_roots.get(path) {
            return cached.clone();
        }
        let root = sessions::workspace_repo_root_resolved(path);
        self.resolved_roots.insert(path.to_string(), root.clone());
        root
    }

    /// The canonical repo identity for `key`: the real filesystem path, with any
    /// worktree suffix stripped. `None` when the key cannot be resolved to a path
    /// (an opaque client id, a directory no longer on disk, or a slug that two
    /// directories fit), leaving the original key as its own identity.
    ///
    /// Decoding is what makes the rollup actually merge. Claude Code writes a
    /// dash-mangled slug and Codex/OpenCode write real paths, so without this the
    /// same repo keeps two identities and the "one row per repo" promise fails.
    pub fn repo_root(&mut self, key: &str) -> Option<String> {
        if let Some(cached) = self.roots.get(key) {
            return cached.clone();
        }
        // Decode first: Claude's slug encodes `.claude/worktrees/` as dashes, so
        // the marker is only visible once the real path is recovered.
        let decoded = self.decoded(key);
        let path = decoded.clone().unwrap_or_else(|| key.to_string());
        let root = self
            .repo_root_of_path(&path)
            // Not a worktree: the decoded path is already the repo identity.
            .or_else(|| decoded.clone())
            // Undecodable slug (deleted worktree): fall back to the repo prefix
            // recovered from the slug string so it still merges with its repo.
            .or_else(|| sessions::workspace_repo_root_from_slug(key));
        self.roots.insert(key.to_string(), root.clone());
        root
    }
}

/// Grouping key, stored key and display label for a message's workspace.
///
/// Shared with the TUI, which runs its own aggregation over the same messages —
/// duplicating this would let the two drift on how worktrees roll up and how
/// Claude Code's dash-mangled keys are labeled.
pub fn workspace_bucket(
    msg: &UnifiedMessage,
    rollup: WorktreeRollup,
    labeler: &mut WorkspaceLabeler,
) -> (String, Option<String>, String) {
    let Some(key) = msg.workspace_key.as_deref() else {
        return (
            UNKNOWN_WORKSPACE_GROUP_KEY.to_string(),
            None,
            UNKNOWN_WORKSPACE_LABEL.to_string(),
        );
    };

    // Under MergeIntoRepo the repo root becomes the grouping identity, so every
    // worktree of a repo lands in one row and the row reports the repo's path.
    if rollup == WorktreeRollup::MergeIntoRepo {
        if let Some(root) = labeler.repo_root(key) {
            let label = labeler.label(&root);
            return (root.clone(), Some(root), label);
        }
    }

    // A parser-supplied label is authoritative — it is the only thing that can
    // name a workspace whose key is not a path (Warp's workspace UUID). Keys
    // that fell back to `workspace_label_from_key` are relabeled, because that
    // helper returns the whole dash-mangled slug for Claude Code.
    let label = match msg.workspace_label.as_deref() {
        Some(label) if Some(label.to_string()) != sessions::workspace_label_from_key(key) => {
            label.to_string()
        }
        _ => labeler.label(key),
    };

    (key.to_string(), Some(key.to_string()), label)
}

/// The label to display for every distinct workspace in `messages`, keyed by the
/// grouping identity its rows will use.
///
/// A label is a basename, so `~/work/api` and `~/oss/api` render as the same
/// text even though they stay separate rows with separate keys — the row is
/// still correct, but the reader cannot tell which repo it is looking at. Each
/// colliding label is qualified here with the fewest leading parent segments
/// that tell the group apart (`work/api`, `oss/api`).
///
/// Grouping keys are never touched: this rewrites display text only, so no usage
/// moves between rows and no total changes.
///
/// Resolved up front rather than as a post-pass over the rows: the daily
/// breakdown keys its legend off the label while it aggregates, so fixing the
/// table afterwards would leave the chart showing the ambiguous name.
pub fn workspace_label_overrides<'a>(
    messages: impl IntoIterator<Item = &'a UnifiedMessage>,
    rollup: WorktreeRollup,
    labeler: &mut WorkspaceLabeler,
) -> HashMap<String, String> {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut base: BTreeMap<String, String> = BTreeMap::new();
    for msg in messages {
        let Some(key) = msg.workspace_key.as_deref() else {
            continue;
        };
        if !seen.insert(key) {
            continue;
        }
        let (group_key, _, label) = workspace_bucket(msg, rollup, labeler);
        base.entry(group_key).or_insert(label);
    }

    disambiguate_workspace_labels(
        base.iter()
            .map(|(key, label)| (key.as_str(), label.as_str())),
        labeler,
    )
}

/// Rewrite `labeled` — (grouping key, base label) pairs — so no two keys share a
/// label, qualifying each collision with as few leading path segments as it
/// takes and falling back to the grouping key when the filesystem cannot
/// separate them at all.
fn disambiguate_workspace_labels<'a>(
    labeled: impl IntoIterator<Item = (&'a str, &'a str)>,
    labeler: &mut WorkspaceLabeler,
) -> HashMap<String, String> {
    // BTree everywhere: with two directories that encode identically there is
    // nothing on disk to order them by, so the output must not depend on hash
    // iteration order.
    let mut by_label: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for (key, label) in labeled {
        by_label.entry(label).or_default().insert(key);
    }

    let mut resolved: BTreeMap<String, String> = BTreeMap::new();
    for (label, keys) in by_label {
        if keys.len() == 1 {
            for key in keys {
                resolved.insert(key.to_string(), label.to_string());
            }
            continue;
        }

        let keys: Vec<&str> = keys.into_iter().collect();
        let parents: Vec<Vec<String>> = keys
            .iter()
            .map(|key| workspace_parent_segments(labeler, key))
            .collect();
        let max_depth = parents.iter().map(Vec::len).max().unwrap_or(0);

        // Fewest segments that tell the most rows apart. Escalating past that
        // buys nothing: when two keys name the SAME directory every remaining
        // segment is identical on both rows, so a deeper qualifier only makes
        // the label longer and pushes the part that actually differs — the key
        // appended below — off a narrow row.
        let mut depth = 0;
        let mut separated = 0;
        for candidate in 0..=max_depth {
            let candidates: HashSet<String> = parents
                .iter()
                .map(|parents| qualify_workspace_label(label, parents, candidate))
                .collect();
            if candidates.len() > separated {
                separated = candidates.len();
                depth = candidate;
            }
            if separated == keys.len() {
                break;
            }
        }

        for (key, parents) in keys.iter().zip(&parents) {
            resolved.insert(
                (*key).to_string(),
                qualify_workspace_label(label, parents, depth),
            );
        }
    }

    // Whatever the filesystem could not separate — two keys that resolve to the
    // same directory, or keys with no path at all — is separated by the grouping
    // key, which is unique by construction.
    let mut duplicates: BTreeMap<&str, usize> = BTreeMap::new();
    for label in resolved.values() {
        *duplicates.entry(label.as_str()).or_default() += 1;
    }
    let ambiguous: HashSet<String> = duplicates
        .into_iter()
        .filter(|(_, count)| *count > 1)
        .map(|(label, _)| label.to_string())
        .collect();

    resolved
        .into_iter()
        .map(|(key, label)| {
            if ambiguous.contains(&label) {
                let qualified = format!("{label} ({key})");
                (key, qualified)
            } else {
                (key, label)
            }
        })
        .collect()
}

/// Parent segments of the directory whose name the label leads with, nearest
/// first. Empty when the key resolves to no path, which is what makes the
/// caller fall through to qualifying by the key itself.
fn workspace_parent_segments(labeler: &mut WorkspaceLabeler, key: &str) -> Vec<String> {
    let Some(path) = labeler.path(key) else {
        return Vec::new();
    };
    // A worktree label reads `repo ⑃ worktree`, so it is the REPO whose parents
    // disambiguate it, not the worktree's `.claude/worktrees` scaffolding.
    let anchor = labeler.repo_root_of_path(&path).unwrap_or(path);
    let mut segments: Vec<String> = anchor
        .split('/')
        .filter(|segment| !segment.is_empty())
        .map(str::to_string)
        .collect();
    // The last segment is already the label's own name.
    segments.pop();
    segments.reverse();
    segments
}

/// `label` prefixed with up to `depth` parent segments, outermost first, so the
/// result reads like the tail of the path it came from.
fn qualify_workspace_label(label: &str, parents: &[String], depth: usize) -> String {
    let taken = depth.min(parents.len());
    if taken == 0 {
        return label.to_string();
    }
    let mut prefix: Vec<&str> = parents[..taken].iter().map(String::as_str).collect();
    prefix.reverse();
    format!("{}/{label}", prefix.join("/"))
}
