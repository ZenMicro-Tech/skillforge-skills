use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};
use skillforge_runtime::{dispatch, Embedded, SkillHandler};
use url::Url;

struct Handler;

// ---------------------------------------------------------------------------
// DSN parsing
// ---------------------------------------------------------------------------

enum Backend {
    Sqlite(String),    // file path
    Postgres(String),  // full connection string
}

fn parse_dsn(dsn: &str) -> Result<Backend> {
    if let Some(path) = dsn.strip_prefix("sqlite://") {
        Ok(Backend::Sqlite(path.to_string()))
    } else if dsn.starts_with("postgres://") || dsn.starts_with("postgresql://") {
        Ok(Backend::Postgres(dsn.to_string()))
    } else {
        Err(anyhow!("Unsupported DSN scheme. Use sqlite:// or postgres://"))
    }
}

// ---------------------------------------------------------------------------
// SQLite helpers
// ---------------------------------------------------------------------------

fn sqlite_query(path: &str, sql: &str, params: &[Value]) -> Result<Value> {
    let conn = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_WRITE | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;

    let mut stmt = conn.prepare(sql)?;
    let col_count = stmt.column_count();
    let columns: Vec<String> = (0..col_count)
        .map(|i| stmt.column_name(i).unwrap_or("?").to_string())
        .collect();

    let sqlite_params: Vec<Box<dyn rusqlite::types::ToSql>> = params
        .iter()
        .map(|v| json_to_sqlite_param(v))
        .collect();

    let param_refs: Vec<&dyn rusqlite::types::ToSql> =
        sqlite_params.iter().map(|b| b.as_ref()).collect();

    if col_count == 0 {
        let affected_rows = stmt.execute(param_refs.as_slice())?;
        return Ok(json!({
            "columns": [],
            "rows": [],
            "row_count": 0,
            "affected_rows": affected_rows
        }));
    }

    let mut rows_out: Vec<Value> = Vec::new();
    let mut rows = stmt.query(param_refs.as_slice())?;
    while let Some(row) = rows.next()? {
        let mut row_vec: Vec<Value> = Vec::with_capacity(col_count);
        for i in 0..col_count {
            let val = sqlite_value_to_json(row, i);
            row_vec.push(val);
        }
        rows_out.push(Value::Array(row_vec));
    }

    let row_count = rows_out.len();
    Ok(json!({
        "columns": columns,
        "rows": rows_out,
        "row_count": row_count
    }))
}

fn json_to_sqlite_param(v: &Value) -> Box<dyn rusqlite::types::ToSql> {
    match v {
        Value::Null => Box::new(rusqlite::types::Null),
        Value::Bool(b) => Box::new(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Box::new(i)
            } else if let Some(f) = n.as_f64() {
                Box::new(f)
            } else {
                Box::new(n.to_string())
            }
        }
        Value::String(s) => Box::new(s.clone()),
        _ => Box::new(v.to_string()),
    }
}

fn sqlite_value_to_json(row: &rusqlite::Row, idx: usize) -> Value {
    // Try types in order: NULL, INTEGER, REAL, TEXT, BLOB
    if let Ok(val) = row.get::<_, rusqlite::types::Value>(idx) {
        match val {
            rusqlite::types::Value::Null => Value::Null,
            rusqlite::types::Value::Integer(i) => json!(i),
            rusqlite::types::Value::Real(f) => json!(f),
            rusqlite::types::Value::Text(s) => json!(s),
            rusqlite::types::Value::Blob(b) => json!(format!("<blob {} bytes>", b.len())),
        }
    } else {
        Value::Null
    }
}

fn sqlite_tables(path: &str) -> Result<Value> {
    let conn = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;

    let mut stmt = conn.prepare(
        "SELECT name, type FROM sqlite_master WHERE type IN ('table', 'view') ORDER BY name",
    )?;
    let mut tables: Vec<Value> = Vec::new();
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(0)?;
        let kind: String = row.get(1)?;
        tables.push(json!({ "name": name, "type": kind }));
    }

    Ok(json!({ "tables": tables }))
}

fn sqlite_describe(path: &str, table: &str) -> Result<Value> {
    let conn = rusqlite::Connection::open_with_flags(
        path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;

    // Validate table name to prevent injection in PRAGMA
    if !table.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err(anyhow!("Invalid table name"));
    }

    let mut stmt = conn.prepare(&format!("PRAGMA table_info(\"{}\")", table))?;
    let mut columns: Vec<Value> = Vec::new();
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        let col_type: String = row.get(2)?;
        let notnull: bool = row.get::<_, i32>(3)? != 0;
        let default: Value = match row.get::<_, rusqlite::types::Value>(4)? {
            rusqlite::types::Value::Null => Value::Null,
            rusqlite::types::Value::Text(s) => json!(s),
            rusqlite::types::Value::Integer(i) => json!(i),
            rusqlite::types::Value::Real(f) => json!(f),
            _ => Value::Null,
        };
        columns.push(json!({
            "name": name,
            "type": col_type,
            "nullable": !notnull,
            "default": default
        }));
    }

    if columns.is_empty() {
        return Err(anyhow!("Table '{}' not found", table));
    }

    Ok(json!({ "columns": columns }))
}

// ---------------------------------------------------------------------------
// PostgreSQL helpers
// ---------------------------------------------------------------------------

/// Returns true only when the caller explicitly opts out of TLS.
fn pg_tls_is_disabled(dsn: &str) -> Result<bool> {
    let url = Url::parse(dsn).context("Invalid PostgreSQL connection string")?;
    let sslmode = url
        .query_pairs()
        .find(|(name, _)| name == "sslmode")
        .map(|(_, value)| value.into_owned());

    match sslmode.as_deref() {
        None | Some("require") | Some("verify-ca") | Some("verify-full") => Ok(false),
        Some("disable") => Ok(true),
        Some(mode) => Err(anyhow!(
            "Unsupported PostgreSQL sslmode '{}'. Use require, verify-ca, verify-full, or disable.",
            mode
        )),
    }
}

/// Builds a TLS connector using the public WebPKI root certificate store.
/// Rustls validates the server certificate and the hostname from the DSN.
fn pg_tls_connector() -> tokio_postgres_rustls::MakeRustlsConnect {
    let root_store =
        rustls::RootCertStore::from_iter(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = rustls::ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    tokio_postgres_rustls::MakeRustlsConnect::new(config)
}

/// Connects with TLS by default. Plaintext is permitted only with
/// `sslmode=disable`, intended solely for trusted local development.
async fn pg_connect(dsn: &str) -> Result<tokio_postgres::Client> {
    if pg_tls_is_disabled(dsn)? {
        let (client, connection) = tokio_postgres::connect(dsn, tokio_postgres::NoTls).await?;
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("postgres connection error: {}", e);
            }
        });
        return Ok(client);
    }

    let (client, connection) = tokio_postgres::connect(dsn, pg_tls_connector()).await?;
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("postgres connection error: {}", e);
        }
    });

    Ok(client)
}

async fn pg_query(dsn: &str, sql: &str, params: &[Value]) -> Result<Value> {
    let client = pg_connect(dsn).await?;

    let pg_params: Vec<Box<dyn tokio_postgres::types::ToSql + Sync>> = params
        .iter()
        .map(|v| json_to_pg_param(v))
        .collect();
    let param_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
        pg_params.iter().map(|b| b.as_ref()).collect();

    let stmt = client.prepare(sql).await?;
    let columns: Vec<String> = stmt.columns().iter().map(|c| c.name().to_string()).collect();

    if columns.is_empty() {
        let affected_rows = client.execute(&stmt, param_refs.as_slice()).await?;
        return Ok(json!({
            "columns": [],
            "rows": [],
            "row_count": 0,
            "affected_rows": affected_rows
        }));
    }

    let rows = client.query(&stmt, param_refs.as_slice()).await?;

    let mut rows_out: Vec<Value> = Vec::with_capacity(rows.len());
    for row in &rows {
        let mut row_vec: Vec<Value> = Vec::with_capacity(columns.len());
        for (i, col) in stmt.columns().iter().enumerate() {
            let val = pg_value_to_json(row, i, col.type_());
            row_vec.push(val);
        }
        rows_out.push(Value::Array(row_vec));
    }

    let row_count = rows_out.len();
    Ok(json!({
        "columns": columns,
        "rows": rows_out,
        "row_count": row_count
    }))
}

async fn pg_tables(dsn: &str) -> Result<Value> {
    let client = pg_connect(dsn).await?;

    let rows = client
        .query(
            "SELECT table_name, table_type FROM information_schema.tables \
             WHERE table_schema = 'public' ORDER BY table_name",
            &[],
        )
        .await?;

    let mut tables: Vec<Value> = Vec::new();
    for row in &rows {
        let name: String = row.get(0);
        let kind: String = row.get(1);
        let normalized_type = if kind == "VIEW" { "view" } else { "table" };
        tables.push(json!({ "name": name, "type": normalized_type }));
    }

    Ok(json!({ "tables": tables }))
}

async fn pg_describe(dsn: &str, table: &str) -> Result<Value> {
    let client = pg_connect(dsn).await?;

    // Validate table name
    if !table.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return Err(anyhow!("Invalid table name"));
    }

    let rows = client
        .query(
            "SELECT column_name, data_type, is_nullable, column_default \
             FROM information_schema.columns \
             WHERE table_schema = 'public' AND table_name = $1 \
             ORDER BY ordinal_position",
            &[&table],
        )
        .await?;

    if rows.is_empty() {
        return Err(anyhow!("Table '{}' not found", table));
    }

    let mut columns: Vec<Value> = Vec::new();
    for row in &rows {
        let name: String = row.get(0);
        let col_type: String = row.get(1);
        let nullable_str: String = row.get(2);
        let default: Option<String> = row.get(3);
        columns.push(json!({
            "name": name,
            "type": col_type,
            "nullable": nullable_str == "YES",
            "default": default
        }));
    }

    Ok(json!({ "columns": columns }))
}

fn json_to_pg_param(v: &Value) -> Box<dyn tokio_postgres::types::ToSql + Sync> {
    match v {
        Value::Null => Box::new(None::<String>),
        Value::Bool(b) => Box::new(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Box::new(i)
            } else if let Some(f) = n.as_f64() {
                Box::new(f)
            } else {
                Box::new(n.to_string())
            }
        }
        Value::String(s) => Box::new(s.clone()),
        _ => Box::new(v.to_string()),
    }
}

fn pg_value_to_json(
    row: &tokio_postgres::Row,
    idx: usize,
    col_type: &tokio_postgres::types::Type,
) -> Value {
    use tokio_postgres::types::Type;

    // Handle NULL for any type
    macro_rules! try_get {
        ($t:ty) => {
            match row.try_get::<_, Option<$t>>(idx) {
                Ok(Some(v)) => return json!(v),
                Ok(None) => return Value::Null,
                Err(_) => {}
            }
        };
    }

    match *col_type {
        Type::BOOL => try_get!(bool),
        Type::INT2 => try_get!(i16),
        Type::INT4 => try_get!(i32),
        Type::INT8 => try_get!(i64),
        Type::FLOAT4 => try_get!(f32),
        Type::FLOAT8 => try_get!(f64),
        Type::TEXT | Type::VARCHAR | Type::BPCHAR | Type::NAME => try_get!(String),
        Type::JSON | Type::JSONB => {
            match row.try_get::<_, Option<serde_json::Value>>(idx) {
                Ok(Some(v)) => return v,
                Ok(None) => return Value::Null,
                Err(_) => {}
            }
        }
        Type::TIMESTAMP | Type::TIMESTAMPTZ => {
            // chrono is not a dependency; fall through to string representation
        }
        _ => {}
    }

    // Fallback: try as String (works for most types via Display)
    match row.try_get::<_, Option<String>>(idx) {
        Ok(Some(s)) => json!(s),
        Ok(None) => Value::Null,
        Err(_) => json!("<unsupported type>"),
    }
}

// ---------------------------------------------------------------------------
// Handler implementation
// ---------------------------------------------------------------------------

impl SkillHandler for Handler {
    fn call(&self, input: Value) -> Result<Value> {
        let operation = input
            .get("operation")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("Missing required field: operation"))?;

        let args = input
            .get("args")
            .ok_or_else(|| anyhow!("Missing required field: args"))?;

        let dsn_str = args
            .get("dsn")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("Missing required field: args.dsn"))?;

        let result = match operation {
            "query" => self.handle_query(dsn_str, args),
            "tables" => self.handle_tables(dsn_str),
            "describe" => self.handle_describe(dsn_str, args),
            other => Err(anyhow!("Unknown operation: {}", other)),
        };

        match result {
            Ok(data) => Ok(json!({ "ok": true, "data": data })),
            Err(e) => Ok(json!({ "ok": false, "error": e.to_string() })),
        }
    }
}

impl Handler {
    fn handle_query(&self, dsn: &str, args: &Value) -> Result<Value> {
        let sql = args
            .get("sql")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("Missing required field: args.sql"))?;

        let params: Vec<Value> = args
            .get("params")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let backend = parse_dsn(dsn)?;
        match backend {
            Backend::Sqlite(path) => sqlite_query(&path, sql, &params),
            Backend::Postgres(conn_str) => {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()?;
                rt.block_on(pg_query(&conn_str, sql, &params))
            }
        }
    }

    fn handle_tables(&self, dsn: &str) -> Result<Value> {
        let backend = parse_dsn(dsn)?;
        match backend {
            Backend::Sqlite(path) => sqlite_tables(&path),
            Backend::Postgres(conn_str) => {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()?;
                rt.block_on(pg_tables(&conn_str))
            }
        }
    }

    fn handle_describe(&self, dsn: &str, args: &Value) -> Result<Value> {
        let table = args
            .get("table")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow!("Missing required field: args.table"))?;

        let backend = parse_dsn(dsn)?;
        match backend {
            Backend::Sqlite(path) => sqlite_describe(&path, table),
            Backend::Postgres(conn_str) => {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()?;
                rt.block_on(pg_describe(&conn_str, table))
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() -> Result<()> {
    let embedded = Embedded {
        manifest_toml: include_str!(concat!(env!("OUT_DIR"), "/skill.toml")),
        prompt_md: include_str!(concat!(env!("OUT_DIR"), "/prompt.md")),
        schema_json: include_str!(concat!(env!("OUT_DIR"), "/schema.json")),
    };
    dispatch(embedded, Handler)
}
