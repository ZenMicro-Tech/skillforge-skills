# git

Structured Git operations for AI agents. Uses libgit2 (bundled) — no system git installation required. All operations return typed JSON.

## Input Format

```json
{
  "operation": "<operation-name>",
  "repo_path": "<path-to-repo>",
  "args": { ... }
}
```

- `repo_path` defaults to `"."` (current directory) if omitted.

## Supported Operations

### `status`
Get the working tree status with files categorized by state.

**Arguments:**
```json
{
  "path": "optional — limit status to a specific path"
}
```

**Returns:**
```json
{
  "staged": [{ "path": "file.rs", "status": "modified" }],
  "unstaged": [{ "path": "other.rs", "status": "modified" }],
  "untracked": [{ "path": "new.rs" }]
}
```

**Example:**
```json
{ "operation": "status" }
{ "operation": "status", "args": { "path": "src/" } }
```

---

### `diff`
Show changes between commits, staging area, and working tree.

**Arguments:**
```json
{
  "path": "optional — limit diff to a specific path",
  "staged": "optional bool — if true, show only staged changes (--cached)",
  "base": "optional — ref to diff against (e.g., 'main', 'HEAD~3')"
}
```

**Returns:**
```json
{ "diff": "<unified diff output>" }
```

**Example:**
```json
{ "operation": "diff", "args": { "staged": true } }
{ "operation": "diff", "args": { "base": "main" } }
{ "operation": "diff", "args": { "path": "src/main.rs" } }
```

---

### `log`
Show commit history with structured output.

**Arguments:**
```json
{
  "max_count": "optional int (default: 10)",
  "path": "optional — limit to commits affecting this path",
  "format": "optional — custom git format string (returns raw output)",
  "since": "optional — date filter (e.g., '2024-01-01', '2 weeks ago')",
  "author": "optional — filter by author name or email"
}
```

**Returns (default format):**
```json
{
  "commits": [
    {
      "hash": "abc123...",
      "author": "Jane Doe",
      "date": "2024-06-15T10:30:00+00:00",
      "message": "feat: add new endpoint"
    }
  ]
}
```

**Example:**
```json
{ "operation": "log", "args": { "max_count": 5 } }
{ "operation": "log", "args": { "since": "1 week ago", "author": "jane" } }
{ "operation": "log", "args": { "path": "src/main.rs" } }
```

---

### `commit`
Create a commit. Optionally stage files first.

**Arguments:**
```json
{
  "message": "required — commit message",
  "paths": "optional array — files to stage before committing"
}
```

**Returns:**
```json
{
  "message": "feat: add logging",
  "hash": "abc123...",
  "output": "git commit output"
}
```

**Example:**
```json
{ "operation": "commit", "args": { "message": "fix: resolve null pointer", "paths": ["src/main.rs"] } }
```

---

### `branch`
List, create, or delete branches.

**Arguments:**
```json
{
  "action": "required — 'list' | 'create' | 'delete'",
  "name": "required for create/delete — branch name"
}
```

**Returns (list):**
```json
{
  "branches": [
    { "name": "main", "current": true },
    { "name": "feature-x", "current": false }
  ]
}
```

**Returns (create):**
```json
{ "created": "feature-y" }
```

**Returns (delete):**
```json
{ "deleted": "old-branch" }
```

**Example:**
```json
{ "operation": "branch", "args": { "action": "list" } }
{ "operation": "branch", "args": { "action": "create", "name": "feature-auth" } }
{ "operation": "branch", "args": { "action": "delete", "name": "old-feature" } }
```

---

### `checkout`
Switch branches or check out a specific ref.

**Arguments:**
```json
{
  "target": "required — branch, tag, or commit hash",
  "create": "optional bool — if true, creates a new branch (-b flag)"
}
```

**Returns:**
```json
{ "checked_out": "feature-x" }
```

**Example:**
```json
{ "operation": "checkout", "args": { "target": "main" } }
{ "operation": "checkout", "args": { "target": "feature-new", "create": true } }
```

---

### `show`
Show a commit or file contents at a specific ref.

**Arguments:**
```json
{
  "ref": "required — commit hash, tag, or branch",
  "path": "optional — file path to show at that ref (uses ref:path syntax)"
}
```

**Returns:**
```json
{ "content": "<commit details or file content>" }
```

**Example:**
```json
{ "operation": "show", "args": { "ref": "HEAD" } }
{ "operation": "show", "args": { "ref": "main", "path": "README.md" } }
{ "operation": "show", "args": { "ref": "abc123" } }
```

---

### `blame`
Show per-line authorship for a file.

**Arguments:**
```json
{
  "path": "required — file to blame",
  "lines": "optional — line range as 'start,end' (e.g., '10,20')"
}
```

**Returns:**
```json
{
  "blame": [
    {
      "hash": "abc123...",
      "author": "Jane Doe",
      "timestamp": "1718450000",
      "line_no": 10,
      "content": "fn main() {"
    }
  ]
}
```

**Example:**
```json
{ "operation": "blame", "args": { "path": "src/main.rs" } }
{ "operation": "blame", "args": { "path": "src/main.rs", "lines": "1,50" } }
```

---

### `stash`
Stash or restore uncommitted changes.

**Arguments:**
```json
{
  "action": "required — 'push' | 'pop' | 'list'",
  "message": "optional — message for stash push"
}
```

**Returns (push/pop):**
```json
{ "result": "Saved working directory and index state..." }
```

**Returns (list):**
```json
{
  "stashes": [
    { "ref": "stash@{0}", "message": "WIP on main: abc123 commit msg" }
  ]
}
```

**Example:**
```json
{ "operation": "stash", "args": { "action": "push", "message": "WIP: halfway done" } }
{ "operation": "stash", "args": { "action": "list" } }
{ "operation": "stash", "args": { "action": "pop" } }
```

## Error Handling

All operations return either:
- `{ "ok": true, "data": { ... } }` on success
- `{ "ok": false, "error": "description of what went wrong" }` on failure

Common errors include non-existent paths, invalid refs, merge conflicts, and permission issues.

## Notes

- This skill is fully standalone — it bundles libgit2 and requires no system git installation.
- The `repo_path` field specifies the repository to operate on. It will search upward from that path to find a `.git` directory.
- Operations that modify the repository (commit, checkout, branch delete, stash pop) should be used with care.
- The `log` operation returns structured commit objects. The `since` filter supports relative expressions (e.g., "2 weeks ago") and ISO dates (YYYY-MM-DD).
- The `blame` operation reads the working tree file for line content.
