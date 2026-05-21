# aws-cli

Read-only access to AWS via the official AWS Rust SDK. Authentication uses the default credential chain (env vars, `~/.aws/credentials`, IMDS, SSO) — same as the `aws` CLI.

## Usage

Call this tool with a `service`, `operation`, and an `args` object. Only read-only operations are supported.

### Supported operations

**S3**
- `s3 list-buckets` — args: `{}`
- `s3 list-objects` — args: `{ "bucket": string, "prefix"?: string, "max_keys"?: number }`
- `s3 head-object` — args: `{ "bucket": string, "key": string }`

**EC2**
- `ec2 describe-instances` — args: `{ "region"?: string }`
- `ec2 describe-regions` — args: `{}`

**STS**
- `sts get-caller-identity` — args: `{}`

**IAM**
- `iam list-users` — args: `{}`
- `iam list-roles` — args: `{}`

### Examples

```json
{ "service": "sts", "operation": "get-caller-identity", "args": {} }
{ "service": "s3", "operation": "list-objects", "args": { "bucket": "my-bucket", "prefix": "logs/" } }
{ "service": "ec2", "operation": "describe-instances", "args": { "region": "us-west-2" } }
```

Output is JSON shaped like `{ "ok": true, "data": <api response> }` on success or `{ "ok": false, "error": <message> }` on failure.
