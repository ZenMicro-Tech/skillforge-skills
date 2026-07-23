# Changelog

All notable changes to the `sql-query` skill are documented in this file.

## [0.3.0] - 2026-07-22

### Changed

- Removed read-only enforcement from `query`; it can now execute a single SQL statement that reads or modifies data and schema.
- SQLite `query` connections now open existing database files with read-write access.
- PostgreSQL `query` statements no longer run inside a `READ ONLY` transaction.
- Non-row-returning statements now include `affected_rows` in their successful result.

## [0.2.0] - 2026-07-21

### Security

- PostgreSQL connections now use TLS by default with public WebPKI certificate and hostname validation.

### Changed

- Plaintext PostgreSQL connections now require the explicit `sslmode=disable` DSN option and are intended only for trusted local development.
- `sslmode=prefer` and `sslmode=allow` are rejected to prevent insecure fallback behavior.
- `sslmode=require`, `sslmode=verify-ca`, and `sslmode=verify-full` use the strict certificate-and-hostname-validated TLS connection.