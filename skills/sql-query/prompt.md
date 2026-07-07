# sql-query

Execute read-only SQL queries against SQLite or PostgreSQL databases and return structured results.

## Operations

### `query`

Execute a read-only SQL query and return columnar results.

**Args:**
- `dsn` (required): Database connection string.
- `sql` (required): The SQL query to execute.
- `params` (optional): Array of positional parameters to bind.

**Returns:**
```json
{ "ok": true, "data": { "columns": ["id", "name"], "rows": [[1, "Alice"], [2, "Bob"]], "row_count": 2 } }
```

**Read-only enforcement:** Queries starting with INSERT, UPDATE, DELETE, DROP, ALTER, CREATE, or TRUNCATE (case-insensitive, after trimming whitespace) are rejected. This is a safety guard, not a security boundary.

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

| Database   | Format                                      | Example                                      |
|------------|---------------------------------------------|----------------------------------------------|
| SQLite     | `sqlite:///path/to/db.sqlite`               | `sqlite:///tmp/mydata.sqlite`                |
| PostgreSQL | `postgres://user:pass@host:port/dbname`     | `postgres://app:secret@localhost:5432/mydb`   |

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

**List tables in PostgreSQL:**
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
