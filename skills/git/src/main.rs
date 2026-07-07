use anyhow::{anyhow, Context, Result};
use git2::{
    BlameOptions, BranchType, DiffOptions, ObjectType, Repository, Signature, Sort,
    StatusOptions, StatusShow,
};
use serde::Deserialize;
use serde_json::{json, Value};
use skillforge_runtime::{dispatch, Embedded, SkillHandler};
use std::path::Path;

#[derive(Debug, Deserialize)]
struct Call {
    operation: String,
    #[serde(default)]
    args: Value,
    #[serde(default = "default_repo_path")]
    repo_path: String,
}

fn default_repo_path() -> String {
    ".".to_string()
}

struct Handler;

impl SkillHandler for Handler {
    fn call(&self, input: Value) -> Result<Value> {
        let call: Call = serde_json::from_value(input).context("invalid input")?;
        let result = handle_operation(&call.operation, call.args, &call.repo_path);
        Ok(match result {
            Ok(data) => json!({ "ok": true, "data": data }),
            Err(e) => json!({ "ok": false, "error": format!("{e:#}") }),
        })
    }
}

fn main() -> Result<()> {
    let embedded = Embedded {
        manifest_toml: include_str!(concat!(env!("OUT_DIR"), "/skill.toml")),
        prompt_md: include_str!(concat!(env!("OUT_DIR"), "/prompt.md")),
        schema_json: include_str!(concat!(env!("OUT_DIR"), "/schema.json")),
    };
    dispatch(embedded, Handler)
}

fn open_repo(repo_path: &str) -> Result<Repository> {
    Repository::discover(repo_path)
        .with_context(|| format!("opening repository at {repo_path}"))
}

fn handle_operation(operation: &str, args: Value, repo_path: &str) -> Result<Value> {
    match operation {
        "status" => op_status(args, &open_repo(repo_path)?),
        "diff" => op_diff(args, &open_repo(repo_path)?),
        "log" => op_log(args, &open_repo(repo_path)?),
        "commit" => op_commit(args, &open_repo(repo_path)?),
        "branch" => op_branch(args, &open_repo(repo_path)?),
        "checkout" => op_checkout(args, &open_repo(repo_path)?),
        "show" => op_show(args, &open_repo(repo_path)?),
        "blame" => op_blame(args, &open_repo(repo_path)?),
        "stash" => op_stash(args, &mut open_repo(repo_path)?),
        other => Err(anyhow!("unsupported operation: {other:?}")),
    }
}

// ---------------------------------------------------------------------------
// status
// ---------------------------------------------------------------------------

fn op_status(args: Value, repo: &Repository) -> Result<Value> {
    #[derive(Deserialize, Default)]
    struct Args {
        #[serde(default)]
        path: Option<String>,
    }
    let a: Args = serde_json::from_value(args).unwrap_or_default();

    let mut opts = StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(true)
        .show(StatusShow::IndexAndWorkdir);

    if let Some(ref p) = a.path {
        opts.pathspec(p);
    }

    let statuses = repo.statuses(Some(&mut opts))?;

    let mut staged = Vec::new();
    let mut unstaged = Vec::new();
    let mut untracked = Vec::new();

    for entry in statuses.iter() {
        let path = entry.path().unwrap_or("").to_string();
        let status = entry.status();

        if status.is_wt_new() {
            untracked.push(json!({ "path": path }));
            continue;
        }

        if status.is_index_new()
            || status.is_index_modified()
            || status.is_index_deleted()
            || status.is_index_renamed()
            || status.is_index_typechange()
        {
            let label = if status.is_index_new() {
                "added"
            } else if status.is_index_modified() {
                "modified"
            } else if status.is_index_deleted() {
                "deleted"
            } else if status.is_index_renamed() {
                "renamed"
            } else {
                "type-changed"
            };
            staged.push(json!({ "path": path, "status": label }));
        }

        if status.is_wt_modified()
            || status.is_wt_deleted()
            || status.is_wt_renamed()
            || status.is_wt_typechange()
        {
            let label = if status.is_wt_modified() {
                "modified"
            } else if status.is_wt_deleted() {
                "deleted"
            } else if status.is_wt_renamed() {
                "renamed"
            } else {
                "type-changed"
            };
            unstaged.push(json!({ "path": path, "status": label }));
        }
    }

    Ok(json!({
        "staged": staged,
        "unstaged": unstaged,
        "untracked": untracked,
    }))
}

// ---------------------------------------------------------------------------
// diff
// ---------------------------------------------------------------------------

fn op_diff(args: Value, repo: &Repository) -> Result<Value> {
    #[derive(Deserialize, Default)]
    struct Args {
        #[serde(default)]
        path: Option<String>,
        #[serde(default)]
        staged: Option<bool>,
        #[serde(default)]
        base: Option<String>,
    }
    let a: Args = serde_json::from_value(args).unwrap_or_default();

    let mut diff_opts = DiffOptions::new();
    if let Some(ref p) = a.path {
        diff_opts.pathspec(p);
    }

    let diff = if a.staged.unwrap_or(false) {
        let head_tree = repo.head()?.peel_to_tree()?;
        repo.diff_tree_to_index(Some(&head_tree), None, Some(&mut diff_opts))?
    } else if let Some(ref base) = a.base {
        let base_obj = repo.revparse_single(base)?;
        let base_tree = base_obj.peel_to_tree()?;
        let head_tree = repo.head()?.peel_to_tree()?;
        repo.diff_tree_to_tree(Some(&base_tree), Some(&head_tree), Some(&mut diff_opts))?
    } else {
        repo.diff_index_to_workdir(None, Some(&mut diff_opts))?
    };

    let mut output = String::new();
    diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
        let prefix = match line.origin() {
            '+' => "+",
            '-' => "-",
            ' ' => " ",
            'H' | 'F' => "",
            _ => "",
        };
        let content = std::str::from_utf8(line.content()).unwrap_or("");
        if matches!(line.origin(), '+' | '-' | ' ') {
            output.push_str(prefix);
        }
        output.push_str(content);
        true
    })?;

    Ok(json!({ "diff": output }))
}

// ---------------------------------------------------------------------------
// log
// ---------------------------------------------------------------------------

fn op_log(args: Value, repo: &Repository) -> Result<Value> {
    #[derive(Deserialize, Default)]
    struct Args {
        #[serde(default)]
        max_count: Option<usize>,
        #[serde(default)]
        path: Option<String>,
        #[serde(default)]
        since: Option<String>,
        #[serde(default)]
        author: Option<String>,
    }
    let a: Args = serde_json::from_value(args).unwrap_or_default();
    let max_count = a.max_count.unwrap_or(10);

    let mut revwalk = repo.revwalk()?;
    revwalk.set_sorting(Sort::TIME)?;
    revwalk.push_head()?;

    let mut commits = Vec::new();

    for oid in revwalk {
        if commits.len() >= max_count {
            break;
        }
        let oid = oid?;
        let commit = repo.find_commit(oid)?;

        if let Some(ref author_filter) = a.author {
            let sig = commit.author();
            let name = sig.name().unwrap_or("");
            let email = sig.email().unwrap_or("");
            let filter_lower = author_filter.to_lowercase();
            if !name.to_lowercase().contains(&filter_lower)
                && !email.to_lowercase().contains(&filter_lower)
            {
                continue;
            }
        }

        if let Some(ref since) = a.since {
            if let Ok(since_ts) = parse_relative_time(since) {
                let commit_time = commit.time().seconds();
                if commit_time < since_ts {
                    break;
                }
            }
        }

        if let Some(ref path) = a.path {
            if !commit_touches_path(repo, &commit, path)? {
                continue;
            }
        }

        let sig = commit.author();
        let time = commit.time();
        let timestamp = time.seconds();

        commits.push(json!({
            "hash": oid.to_string(),
            "author": sig.name().unwrap_or(""),
            "email": sig.email().unwrap_or(""),
            "date": format_timestamp(timestamp, time.offset_minutes()),
            "message": commit.summary().unwrap_or(""),
        }));
    }

    Ok(json!({ "commits": commits }))
}

fn commit_touches_path(
    repo: &Repository,
    commit: &git2::Commit,
    path: &str,
) -> Result<bool> {
    let tree = commit.tree()?;
    if tree.get_path(Path::new(path)).is_err() {
        return Ok(false);
    }
    if commit.parent_count() == 0 {
        return Ok(true);
    }
    let parent = commit.parent(0)?;
    let parent_tree = parent.tree()?;
    let mut diff_opts = DiffOptions::new();
    diff_opts.pathspec(path);
    let diff = repo.diff_tree_to_tree(Some(&parent_tree), Some(&tree), Some(&mut diff_opts))?;
    Ok(diff.deltas().count() > 0)
}

fn parse_relative_time(s: &str) -> Result<i64> {
    // Simple parser for common relative time expressions
    // Returns a unix timestamp
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs() as i64;

    let s = s.trim().to_lowercase();

    if let Some(rest) = s.strip_suffix(" ago") {
        let parts: Vec<&str> = rest.split_whitespace().collect();
        if parts.len() == 2 {
            if let Ok(n) = parts[0].parse::<i64>() {
                let seconds = match parts[1].trim_end_matches('s') {
                    "second" => 1,
                    "minute" => 60,
                    "hour" => 3600,
                    "day" => 86400,
                    "week" => 604800,
                    "month" => 2592000,
                    "year" => 31536000,
                    _ => return Err(anyhow!("unknown time unit: {}", parts[1])),
                };
                return Ok(now - n * seconds);
            }
        }
    }

    // Try ISO date parsing (YYYY-MM-DD)
    if s.len() == 10 && s.chars().nth(4) == Some('-') {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() == 3 {
            let year: i64 = parts[0].parse().unwrap_or(1970);
            let month: i64 = parts[1].parse().unwrap_or(1);
            let day: i64 = parts[2].parse().unwrap_or(1);
            // Rough approximation
            let days_since_epoch =
                (year - 1970) * 365 + (month - 1) * 30 + (day - 1);
            return Ok(days_since_epoch * 86400);
        }
    }

    Err(anyhow!("cannot parse time expression: {s}"))
}

fn format_timestamp(seconds: i64, offset_minutes: i32) -> String {
    let total_seconds = seconds + (offset_minutes as i64 * 60);
    let days = total_seconds / 86400;
    let remaining = total_seconds % 86400;
    let hours = remaining / 3600;
    let mins = (remaining % 3600) / 60;
    let secs = remaining % 60;

    // Convert days since epoch to date
    let (year, month, day) = days_to_date(days);

    let offset_h = offset_minutes / 60;
    let offset_m = (offset_minutes % 60).unsigned_abs();
    let sign = if offset_minutes >= 0 { '+' } else { '-' };

    format!(
        "{year:04}-{month:02}-{day:02}T{hours:02}:{mins:02}:{secs:02}{sign}{:02}:{:02}",
        offset_h.unsigned_abs(),
        offset_m
    )
}

fn days_to_date(days_since_epoch: i64) -> (i64, i64, i64) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let z = days_since_epoch + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

// ---------------------------------------------------------------------------
// commit
// ---------------------------------------------------------------------------

fn op_commit(args: Value, repo: &Repository) -> Result<Value> {
    #[derive(Deserialize)]
    struct Args {
        message: String,
        #[serde(default)]
        paths: Option<Vec<String>>,
    }
    let a: Args = serde_json::from_value(args).context("commit requires 'message'")?;

    let mut index = repo.index()?;

    if let Some(ref paths) = a.paths {
        for p in paths {
            index.add_path(Path::new(p))?;
        }
        index.write()?;
    }

    let tree_oid = index.write_tree()?;
    let tree = repo.find_tree(tree_oid)?;

    let sig = repo
        .signature()
        .unwrap_or_else(|_| Signature::now("skillforge", "skillforge@localhost").unwrap());

    let parent = match repo.head() {
        Ok(head) => Some(head.peel_to_commit()?),
        Err(_) => None,
    };
    let parents: Vec<&git2::Commit> = parent.iter().collect();

    let oid = repo.commit(Some("HEAD"), &sig, &sig, &a.message, &tree, &parents)?;

    Ok(json!({
        "message": a.message,
        "hash": oid.to_string(),
    }))
}

// ---------------------------------------------------------------------------
// branch
// ---------------------------------------------------------------------------

fn op_branch(args: Value, repo: &Repository) -> Result<Value> {
    #[derive(Deserialize)]
    struct Args {
        action: String,
        #[serde(default)]
        name: Option<String>,
    }
    let a: Args = serde_json::from_value(args).context("branch requires 'action'")?;

    match a.action.as_str() {
        "list" => {
            let branches = repo.branches(Some(BranchType::Local))?;
            let head_ref = repo.head().ok().and_then(|h| h.resolve().ok());
            let head_name = head_ref.as_ref().and_then(|r| r.shorthand().map(String::from));

            let branch_list: Vec<Value> = branches
                .filter_map(|b| b.ok())
                .map(|(branch, _)| {
                    let name = branch.name().ok().flatten().unwrap_or("").to_string();
                    let is_current = head_name.as_deref() == Some(name.as_str());
                    json!({ "name": name, "current": is_current })
                })
                .collect();
            Ok(json!({ "branches": branch_list }))
        }
        "create" => {
            let name =
                a.name.as_deref().ok_or_else(|| anyhow!("branch create requires 'name'"))?;
            let head_commit = repo.head()?.peel_to_commit()?;
            repo.branch(name, &head_commit, false)?;
            Ok(json!({ "created": name }))
        }
        "delete" => {
            let name =
                a.name.as_deref().ok_or_else(|| anyhow!("branch delete requires 'name'"))?;
            let mut branch = repo.find_branch(name, BranchType::Local)?;
            branch.delete()?;
            Ok(json!({ "deleted": name }))
        }
        other => Err(anyhow!("unsupported branch action: {other:?}")),
    }
}

// ---------------------------------------------------------------------------
// checkout
// ---------------------------------------------------------------------------

fn op_checkout(args: Value, repo: &Repository) -> Result<Value> {
    #[derive(Deserialize)]
    struct Args {
        target: String,
        #[serde(default)]
        create: Option<bool>,
    }
    let a: Args = serde_json::from_value(args).context("checkout requires 'target'")?;

    if a.create.unwrap_or(false) {
        let head_commit = repo.head()?.peel_to_commit()?;
        let branch = repo.branch(&a.target, &head_commit, false)?;
        let branch_ref = branch.into_reference();
        repo.set_head(branch_ref.name().unwrap_or("HEAD"))?;
        repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))?;
    } else {
        let obj = repo.revparse_single(&a.target)?;
        repo.checkout_tree(&obj, Some(git2::build::CheckoutBuilder::new().force()))?;
        if repo.find_branch(&a.target, BranchType::Local).is_ok() {
            repo.set_head(&format!("refs/heads/{}", a.target))?;
        } else {
            repo.set_head_detached(obj.id())?;
        }
    }

    let head = repo.head()?;
    let checked_out = head.shorthand().unwrap_or("HEAD").to_string();
    Ok(json!({ "checked_out": checked_out }))
}

// ---------------------------------------------------------------------------
// show
// ---------------------------------------------------------------------------

fn op_show(args: Value, repo: &Repository) -> Result<Value> {
    #[derive(Deserialize)]
    struct Args {
        #[serde(rename = "ref")]
        git_ref: String,
        #[serde(default)]
        path: Option<String>,
    }
    let a: Args = serde_json::from_value(args).context("show requires 'ref'")?;

    let obj = repo.revparse_single(&a.git_ref)?;

    if let Some(ref path) = a.path {
        let tree = obj.peel_to_tree()?;
        let entry = tree.get_path(Path::new(path))?;
        let blob = repo.find_blob(entry.id())?;
        let content = String::from_utf8_lossy(blob.content()).to_string();
        Ok(json!({ "content": content }))
    } else {
        match obj.kind() {
            Some(ObjectType::Commit) => {
                let commit = obj.peel_to_commit()?;
                let sig = commit.author();
                Ok(json!({
                    "type": "commit",
                    "hash": commit.id().to_string(),
                    "author": sig.name().unwrap_or(""),
                    "email": sig.email().unwrap_or(""),
                    "date": format_timestamp(commit.time().seconds(), commit.time().offset_minutes()),
                    "message": commit.message().unwrap_or(""),
                }))
            }
            Some(ObjectType::Blob) => {
                let blob = obj.peel_to_blob()?;
                let content = String::from_utf8_lossy(blob.content()).to_string();
                Ok(json!({ "type": "blob", "content": content }))
            }
            Some(ObjectType::Tree) => {
                let tree = obj.peel_to_tree()?;
                let entries: Vec<Value> = tree
                    .iter()
                    .map(|e| {
                        json!({
                            "name": e.name().unwrap_or(""),
                            "kind": match e.kind() {
                                Some(ObjectType::Blob) => "blob",
                                Some(ObjectType::Tree) => "tree",
                                _ => "other",
                            },
                        })
                    })
                    .collect();
                Ok(json!({ "type": "tree", "entries": entries }))
            }
            _ => Ok(json!({ "type": "unknown", "id": obj.id().to_string() })),
        }
    }
}

// ---------------------------------------------------------------------------
// blame
// ---------------------------------------------------------------------------

fn op_blame(args: Value, repo: &Repository) -> Result<Value> {
    #[derive(Deserialize)]
    struct Args {
        path: String,
        #[serde(default)]
        lines: Option<String>,
    }
    let a: Args = serde_json::from_value(args).context("blame requires 'path'")?;

    let mut blame_opts = BlameOptions::new();

    if let Some(ref lines) = a.lines {
        let parts: Vec<&str> = lines.split(',').collect();
        if parts.len() == 2 {
            if let (Ok(start), Ok(end)) = (parts[0].parse::<usize>(), parts[1].parse::<usize>()) {
                blame_opts.min_line(start);
                blame_opts.max_line(end);
            }
        }
    }

    let blame = repo.blame_file(Path::new(&a.path), Some(&mut blame_opts))?;

    // Read the file to get line content
    let workdir = repo.workdir().ok_or_else(|| anyhow!("bare repository"))?;
    let file_path = workdir.join(&a.path);
    let file_content = std::fs::read_to_string(&file_path)
        .unwrap_or_default();
    let lines: Vec<&str> = file_content.lines().collect();

    let mut entries = Vec::new();
    for hunk_idx in 0..blame.len() {
        let hunk = blame.get_index(hunk_idx).unwrap();
        let sig = hunk.final_signature();
        let start_line = hunk.final_start_line();
        let line_count = hunk.lines_in_hunk();

        for i in 0..line_count {
            let line_no = start_line + i;
            let content = lines.get(line_no - 1).unwrap_or(&"");
            entries.push(json!({
                "hash": hunk.final_commit_id().to_string(),
                "author": sig.name().unwrap_or(""),
                "timestamp": hunk.final_signature().when().seconds().to_string(),
                "line_no": line_no,
                "content": content,
            }));
        }
    }

    Ok(json!({ "blame": entries }))
}

// ---------------------------------------------------------------------------
// stash
// ---------------------------------------------------------------------------

fn op_stash(args: Value, repo: &mut Repository) -> Result<Value> {
    #[derive(Deserialize)]
    struct Args {
        action: String,
        #[serde(default)]
        message: Option<String>,
    }
    let a: Args = serde_json::from_value(args).context("stash requires 'action'")?;

    match a.action.as_str() {
        "push" => {
            let sig = repo
                .signature()
                .unwrap_or_else(|_| Signature::now("skillforge", "skillforge@localhost").unwrap());
            let msg = a.message.as_deref();
            let oid = repo.stash_save(&sig, msg.unwrap_or(""), None)?;
            Ok(json!({ "result": format!("Stashed as {}", oid) }))
        }
        "pop" => {
            repo.stash_pop(0, None)?;
            Ok(json!({ "result": "Applied and dropped stash@{0}" }))
        }
        "list" => {
            let mut stashes = Vec::new();
            repo.stash_foreach(|index, message, _oid| {
                stashes.push(json!({
                    "ref": format!("stash@{{{index}}}"),
                    "message": message,
                }));
                true
            })?;
            Ok(json!({ "stashes": stashes }))
        }
        other => Err(anyhow!("unsupported stash action: {other:?}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handler_invalid_input() {
        let handler = Handler;
        let result = handler.call(json!({}));
        assert!(result.is_err());
    }

    #[test]
    fn test_days_to_date() {
        let (y, m, d) = days_to_date(0);
        assert_eq!((y, m, d), (1970, 1, 1));
    }

    #[test]
    fn test_format_timestamp() {
        let s = format_timestamp(0, 0);
        assert_eq!(s, "1970-01-01T00:00:00+00:00");
    }

    #[test]
    fn test_parse_relative_time() {
        let result = parse_relative_time("2024-01-15");
        assert!(result.is_ok());
    }
}
