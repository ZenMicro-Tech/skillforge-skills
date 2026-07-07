# github

A skillforge skill that provides GitHub REST API operations for AI agents. Supports repository info, issues, pull requests, reviews, workflow runs, and releases.

## Authentication

Set one of these environment variables before invoking the skill:

- `GITHUB_TOKEN` (preferred)
- `GH_TOKEN` (fallback)

The token needs appropriate scopes for the operations you intend to use (e.g., `repo` for private repos, `public_repo` for public repos).

## Operations

### get-repo

Get repository information.

```json
{ "operation": "get-repo", "args": { "owner": "octocat", "repo": "hello-world" } }
```

### list-issues

List issues for a repository.

```json
{ "operation": "list-issues", "args": { "owner": "octocat", "repo": "hello-world", "state": "open", "labels": "bug,enhancement", "per_page": 30 } }
```

Optional args: `state` (open|closed|all), `labels` (comma-separated), `per_page`.

### get-issue

Get a single issue by number.

```json
{ "operation": "get-issue", "args": { "owner": "octocat", "repo": "hello-world", "number": 42 } }
```

### create-issue

Create a new issue.

```json
{ "operation": "create-issue", "args": { "owner": "octocat", "repo": "hello-world", "title": "Bug report", "body": "Description of the bug", "labels": ["bug"], "assignees": ["octocat"] } }
```

Required: `owner`, `repo`, `title`. Optional: `body`, `labels` (array), `assignees` (array).

### list-pulls

List pull requests for a repository.

```json
{ "operation": "list-pulls", "args": { "owner": "octocat", "repo": "hello-world", "state": "open", "per_page": 10 } }
```

Optional args: `state` (open|closed|all), `per_page`.

### get-pull

Get a single pull request by number.

```json
{ "operation": "get-pull", "args": { "owner": "octocat", "repo": "hello-world", "number": 7 } }
```

### create-pull

Create a new pull request.

```json
{ "operation": "create-pull", "args": { "owner": "octocat", "repo": "hello-world", "title": "Add feature", "body": "This PR adds a new feature.", "head": "feature-branch", "base": "main" } }
```

Required: `owner`, `repo`, `title`, `head`, `base`. Optional: `body`.

### list-pull-files

List files changed in a pull request.

```json
{ "operation": "list-pull-files", "args": { "owner": "octocat", "repo": "hello-world", "number": 7 } }
```

### create-review-comment

Create an inline review comment on a pull request.

```json
{ "operation": "create-review-comment", "args": { "owner": "octocat", "repo": "hello-world", "number": 7, "body": "Consider using a constant here.", "path": "src/main.rs", "line": 15 } }
```

Required: `owner`, `repo`, `number`, `body`, `path`, `line`.

### list-runs

List recent workflow runs (GitHub Actions).

```json
{ "operation": "list-runs", "args": { "owner": "octocat", "repo": "hello-world", "per_page": 5 } }
```

Optional: `per_page`.

### list-releases

List releases for a repository.

```json
{ "operation": "list-releases", "args": { "owner": "octocat", "repo": "hello-world", "per_page": 10 } }
```

Optional: `per_page`.

## Response Format

All operations return:

```json
{ "ok": true, "data": { ... } }
```

On error:

```json
{ "ok": false, "error": "GitHub API returned 404: Not Found" }
```

## Examples

**List open bugs in a repo:**
```json
{ "operation": "list-issues", "args": { "owner": "rust-lang", "repo": "rust", "state": "open", "labels": "C-bug", "per_page": 5 } }
```

**Check recent CI runs:**
```json
{ "operation": "list-runs", "args": { "owner": "my-org", "repo": "my-app", "per_page": 3 } }
```

**Create a PR:**
```json
{ "operation": "create-pull", "args": { "owner": "my-org", "repo": "my-app", "title": "fix: resolve null pointer in parser", "body": "Fixes #123", "head": "fix/null-pointer", "base": "main" } }
```
