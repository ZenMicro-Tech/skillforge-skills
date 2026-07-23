# Changelog

All notable changes to the `kubernetes` skill are documented in this file.

## [0.1.0] - 2026-07-22

### Added

- Read-only Kubernetes cluster queries through the locally installed `kubectl` CLI.
- Structured `get`, `describe`, `logs`, and `api-resources` operations.
- Kubeconfig and context selection that retains `kubectl`'s normal authentication and RBAC behavior.
- Input validation that prevents arbitrary `kubectl` arguments and mutating operations.