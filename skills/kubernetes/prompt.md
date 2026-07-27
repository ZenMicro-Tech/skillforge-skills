# kubernetes

Query and modify Kubernetes clusters through structured operations or direct `kubectl` commands.

## Authentication and cluster selection

The skill invokes the locally installed `kubectl` executable. Authentication, authorization, API endpoint selection, and credential refresh are handled by `kubectl` through its normal kubeconfig behavior.

- By default, it uses the active `kubectl` context and standard kubeconfig resolution.
- Set `args.context` to select a named kubeconfig context.
- Set `args.kubeconfig` to select a specific kubeconfig file.
- The identity can access only resources permitted by its Kubernetes RBAC permissions.

Do not place tokens, certificates, or credentials directly in the input.

## Capabilities

The structured `get`, `describe`, `logs`, and `api-resources` operations provide predictable output for common queries. The `kubectl` operation accepts arbitrary arguments and supports the full CLI, including mutating commands such as `apply`, `create`, `delete`, `patch`, and `scale`.

Use mutating commands only when requested and review the target context, namespace, resource, and command arguments before execution. Kubernetes RBAC permissions still apply.

## Operations

### `get`

Retrieve one resource or a list of resources as parsed Kubernetes JSON.

**Args:**
- `resource` (required): Resource type, for example `pods`, `deployments`, `services`, `configmaps`, or `nodes`.
- `name` (optional): Resource name.
- `namespace` (optional): Namespace to query.
- `all_namespaces` (optional): Set to `true` to list across namespaces. Cannot be combined with `namespace`.
- `label_selector` (optional): Label selector, such as `app=api`.
- `field_selector` (optional): Field selector.
- `context` (optional): Kubeconfig context.
- `kubeconfig` (optional): Path to a kubeconfig file.

**Returns:**
```json
{
  "ok": true,
  "data": {
    "apiVersion": "v1",
    "kind": "PodList",
    "items": []
  }
}
```

### `describe`

Return the human-readable description of one named resource.

**Args:**
- `resource` (required): Resource type.
- `name` (required): Resource name.
- `namespace` (optional): Namespace containing the resource.
- `context` (optional): Kubeconfig context.
- `kubeconfig` (optional): Path to a kubeconfig file.

**Returns:**
```json
{ "ok": true, "data": { "text": "Name: ...\nNamespace: ...\n" } }
```

### `logs`

Return logs from a pod container.

**Args:**
- `pod` (required): Pod name.
- `namespace` (optional): Pod namespace.
- `container` (optional): Container name for multi-container pods.
- `tail_lines` (optional): Maximum number of returned lines, from 1 to 10,000.
- `since` (optional): Duration window, such as `15m` or `1h`.
- `previous` (optional): Set to `true` to retrieve logs from the previous container instance.
- `context` (optional): Kubeconfig context.
- `kubeconfig` (optional): Path to a kubeconfig file.

**Returns:**
```json
{ "ok": true, "data": { "text": "2026-07-22T12:00:00Z server started\n" } }
```

### `api-resources`

List resource types known by the selected cluster.

**Args:**
- `context` (optional): Kubeconfig context.
- `kubeconfig` (optional): Path to a kubeconfig file.

**Returns:**
```json
{ "ok": true, "data": { "resources": ["pods", "deployments.apps", "services"] } }
```

### `kubectl`

Run any `kubectl` command, including commands that modify cluster resources.

**Args:**
- `command` (required): Non-empty array of arguments passed directly to `kubectl`, excluding the executable name.
- `context` (optional): Kubeconfig context, prepended as a global option.
- `kubeconfig` (optional): Path to a kubeconfig file, prepended as a global option.

The command is executed directly without a shell. Shell operators such as pipes and redirections are not interpreted.

**Returns:**
```json
{ "ok": true, "data": { "text": "deployment.apps/api scaled\n" } }
```

## Examples

**Scale a deployment:**
```json
{
  "operation": "kubectl",
  "args": {
    "command": ["scale", "deployment/api", "--replicas=3", "--namespace=production"],
    "context": "production"
  }
}
```

**Apply a manifest:**
```json
{
  "operation": "kubectl",
  "args": {
    "command": ["apply", "--filename=deployment.yaml"],
    "context": "production"
  }
}
```

**List pods in a namespace:**
```json
{
  "operation": "get",
  "args": {
    "resource": "pods",
    "namespace": "production",
    "label_selector": "app=api"
  }
}
```

**Inspect a deployment using a selected context:**
```json
{
  "operation": "describe",
  "args": {
    "resource": "deployment",
    "name": "api",
    "namespace": "production",
    "context": "production"
  }
}
```

**Read the latest 200 container log lines:**
```json
{
  "operation": "logs",
  "args": {
    "pod": "api-5bd99d898d-x7p6q",
    "namespace": "production",
    "container": "api",
    "tail_lines": 200
  }
}
```

## Error response

On failure, the skill returns:

```json
{ "ok": false, "error": "description of the kubectl error" }
```
