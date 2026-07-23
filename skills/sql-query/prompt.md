# sql-query

Execute SQL statements against SQLite or PostgreSQL databases and return structured results.

## Operations

### `query`

Execute a single SQL statement and return structured results.

**Args:**
- `dsn` (required): Database connection string.
- `sql` (required): A single SQL statement to execute.
- `params` (optional): Array of positional parameters to bind.

**Returns:**
```json
{ "ok": true, "data": { "columns": ["id", "name"], "rows": [[1, "Alice"], [2, "Bob"]], "row_count": 2 } }
```

**Result behavior:**
- Statements that return rows, including `SELECT` and `INSERT ... RETURNING`, return `columns`, `rows`, and `row_count`.
- Statements that do not return rows, including `INSERT`, `UPDATE`, `DELETE`, and DDL, return empty `columns` and `rows`, `row_count: 0`, and `affected_rows`.

**Write behavior:** `query` may read or modify data and schema. Statements execute using the privileges granted by the SQLite file or PostgreSQL credentials in the DSN. Use a least-privilege database role and parameter binding for untrusted values.

### `tables`

List all tables and views in the database.

**Args:**
- `dsn` (required): Database connection string.

**Returns:**
```json
{ "ok": true, "data": { "tables": [{ "name": "users", "type": "table" }, { "name": "active_users", "type": "view" }] } }
```

### `describe`

Describe the columns of a specific table.

**Args:**
- `dsn` (required): Database connection string.
- `table` (required): Name of the table to describe.

**Returns:**
```json
{ "ok": true, "data": { "columns": [{ "name": "id", "type": "INTEGER", "nullable": false, "default": null }, { "name": "email", "type": "TEXT", "nullable": true, "default": null }] } }
```

## DSN Formats

| Database   | Format                                                 | Example                                                   |
|------------|--------------------------------------------------------|-----------------------------------------------------------|
| SQLite     | `sqlite:///path/to/db.sqlite`                          | `sqlite:///tmp/mydata.sqlite`                             |
| PostgreSQL | `postgres://user:pass@host:port/dbname[?sslmode=...]` | `postgres://app:secret@db.example.com:5432/mydb`          |

## PostgreSQL TLS

PostgreSQL connections use TLS by default. The skill validates the server certificate
against public WebPKI certificate authorities and validates that its hostname matches
the host in the DSN.

- Omit `sslmode` for the secure default.
- `sslmode=require`, `sslmode=verify-ca`, and `sslmode=verify-full` are accepted and
  use the same certificate-and-hostname-validated TLS connection.
- Use `sslmode=disable` **only for trusted local development** to explicitly opt out
  of TLS, for example: `postgres://app:pass@localhost:5432/mydb?sslmode=disable`.
- `sslmode=prefer` and `sslmode=allow` are rejected because they can permit insecure
  fallback behavior.

## Error Response

On failure, the skill returns:
```json
{ "ok": false, "error": "description of the error" }
```

## Examples

**Query a SQLite database:**
```json
{
  "operation": "query",
  "args": {
    "dsn": "sqlite:///tmp/app.db",
    "sql": "SELECT id, name FROM users WHERE age > ?",
    "params": [21]
  }
}
```

**Update a SQLite record:**
```json
{
  "operation": "query",
  "args": {
    "dsn": "sqlite:///tmp/app.db",
    "sql": "UPDATE users SET active = ? WHERE id = ?",
    "params": [true, 42]
  }
}
```

**List tables in PostgreSQL over TLS:**
```json
{
  "operation": "tables",
  "args": {
    "dsn": "postgres://admin:pass@db.example.com:5432/production"
  }
}
```

**Describe a table:**
```json
{
  "operation": "describe",
  "args": {
    "dsn": "sqlite:///tmp/app.db",
    "table": "users"
  }
}
```
