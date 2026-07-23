# skillforge-skills

Reference and example skills for [skillforge](https://github.com/zenmicro-tech/ai-skills-platform).

## Skills

- **[git](skills/git/)** — structured Git operations: status, diff, log, commit, branch, blame, stash.
- **[github](skills/github/)** — GitHub API: issues, PRs, reviews, repos, and actions via REST.
- **[kubernetes](skills/kubernetes/)** — read-only Kubernetes cluster inspection through structured `kubectl` operations.
- **[http-client](skills/http-client/)** — generic REST client with auth, headers, query params, and body.
- **[sql-query](skills/sql-query/)** — SQL statements against SQLite or PostgreSQL with structured results.
- **[web-fetch](skills/web-fetch/)** — fetch URLs and convert HTML to markdown/text for LLM consumption.

## Authoring a skill

You'll need the `skillforge` CLI from the [platform repo](https://github.com/zenmicro-tech/ai-skills-platform) on PATH.

```sh
skillforge new my-skill
cd my-skill
# Edit src/main.rs, prompt.md, schema.json
skillforge add ./my-skill        # build + install into all detected agents
skillforge publish my-skill      # push to OCI registry (requires [publish] in skill.toml)
```

## Layout

Each skill is its own Cargo package with a `[workspace]` opt-out so it compiles independently of any parent workspace:

```
my-skill/
  skill.toml      # name, version, runtime, [publish].repo
  prompt.md       # LLM-facing instructions
  schema.json     # JSON Schema for tool input
  Cargo.toml      # has empty `[workspace]` to opt out
  build.rs        # embeds toml/md/json into the binary
  src/main.rs     # implements skillforge_runtime::SkillHandler
```

## Path dependency on skillforge-runtime

Skills depend on `skillforge-runtime` from the platform repo. Today this is a path dep:

```toml
[dependencies]
skillforge-runtime = { path = "../../../ai-skills-platform/crates/skillforge-runtime" }
```

This assumes you've checked out both repos as siblings. When `skillforge-runtime` is published to crates.io, this becomes a normal version dep.
