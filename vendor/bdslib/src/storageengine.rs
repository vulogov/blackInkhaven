use crate::common::error::{Error as EasyError, Result};
use duckdb::{DuckdbConnectionManager, Error as DuckError, Row};
use r2d2::Pool;
use rust_dynamic::value::Value as DynamicValue;
use scheduled_thread_pool::ScheduledThreadPool;
use std::path::Path;
use std::sync::{Arc, OnceLock};

/// Process-wide shared r2d2 maintenance pool.
///
/// r2d2's default is to create a fresh `ScheduledThreadPool` with 3 threads
/// per `Pool::build()` call.  With many DuckDB pools open (one per StorageEngine,
/// many StorageEngines per Shard, many shards) that number compounds quickly and
/// can exhaust the OS thread limit (RLIMIT_NPROC / EAGAIN).
///
/// By sharing a single pool we keep the r2d2 maintenance thread count constant
/// regardless of how many connection pools are open.
///
/// Call [`init_r2d2_thread_pool`] once at startup (before any `StorageEngine` is
/// constructed) to set the thread count from config.  If it has not been called
/// by the time the first pool is needed, a fallback of 3 threads is used.
static R2D2_THREAD_POOL: OnceLock<Arc<ScheduledThreadPool>> = OnceLock::new();

/// Initialise the shared r2d2 thread pool with `num_threads` worker threads.
///
/// Must be called before any [`StorageEngine`] is constructed.  Subsequent calls
/// are no-ops (the pool is already set); the first call wins.
pub(crate) fn init_r2d2_thread_pool(num_threads: usize) {
    let _ = R2D2_THREAD_POOL.set(Arc::new(
        ScheduledThreadPool::builder()
            .num_threads(num_threads.max(1))
            .thread_name_pattern("r2d2-worker-{}")
            .build(),
    ));
}

fn shared_r2d2_thread_pool() -> Arc<ScheduledThreadPool> {
    R2D2_THREAD_POOL
        .get_or_init(|| {
            Arc::new(
                ScheduledThreadPool::builder()
                    .num_threads(3)
                    .thread_name_pattern("r2d2-worker-{}")
                    .build(),
            )
        })
        .clone()
}

pub struct StorageEngine {
    pool: Pool<DuckdbConnectionManager>,
}

impl StorageEngine {
    pub fn new<P: AsRef<Path>>(path: P, init_sql: &'static str, pool_size: u32) -> Result<Self> {
        let manager = DuckdbConnectionManager::file(path)
            .map_err(|e| EasyError::new("Failed to create connection manager", e))?;

        let pool = Pool::builder()
            .max_size(pool_size)
            .thread_pool(shared_r2d2_thread_pool())
            .build(manager)
            .map_err(|e| EasyError::new("Failed to initialize connection pool", e))?;

        // Initialize schema using a temporary connection from the pool
        {
            let conn = pool
                .get()
                .map_err(|e| EasyError::new("Could not get init connection", e))?;
            conn.execute_batch(init_sql)
                .map_err(|e| EasyError::new("Initialization SQL failed", e))?;
        }

        Ok(Self { pool })
    }

    fn map_to_duck<E: std::fmt::Display>(e: E) -> DuckError {
        let safe_err = std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string());
        DuckError::ToSqlConversionFailure(Box::new(safe_err))
    }

    fn row_to_dynamic(row: &Row) -> duckdb::Result<Vec<DynamicValue>> {
        let column_count = row.as_ref().column_count();
        let mut values = Vec::with_capacity(column_count);

        for i in 0..column_count {
            let duck_val = row.get::<_, duckdb::types::Value>(i)?;

            let val = match duck_val {
                duckdb::types::Value::Boolean(b) => {
                    DynamicValue::from(b).map_err(Self::map_to_duck)?
                }
                duckdb::types::Value::Int(iv) => {
                    DynamicValue::from(iv as i64).map_err(Self::map_to_duck)?
                }
                duckdb::types::Value::BigInt(iv) => {
                    DynamicValue::from(iv).map_err(Self::map_to_duck)?
                }
                duckdb::types::Value::Float(f) => {
                    DynamicValue::from(f as f64).map_err(Self::map_to_duck)?
                }
                duckdb::types::Value::Double(d) => {
                    DynamicValue::from(d).map_err(Self::map_to_duck)?
                }
                duckdb::types::Value::Text(t) => {
                    DynamicValue::from(t).map_err(Self::map_to_duck)?
                }
                duckdb::types::Value::Blob(b) => DynamicValue::from_bin(b),
                _ => DynamicValue::nodata(),
            };
            values.push(val);
        }
        Ok(values)
    }

    pub fn select_all(&self, sql: &str) -> Result<Vec<Vec<DynamicValue>>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| EasyError::new("Pool checkout failed for select_all", e))?;

        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| EasyError::new("Query preparation failed", e))?;

        let rows = stmt
            .query_map([], |row| Self::row_to_dynamic(row))
            .map_err(|e| EasyError::new("Execution of select_all failed", e))?;

        let mut results: Vec<Vec<DynamicValue>> = Vec::new();
        for row_result in rows {
            let row: Vec<DynamicValue> = row_result.map_err(|e| EasyError::new("Error fetching row", e))?;
            results.push(row);
        }
        Ok(results)
    }

    pub fn select_foreach<F>(&self, sql: &str, mut f: F) -> Result<()>
    where
        F: FnMut(Vec<DynamicValue>) -> Result<()>,
    {
        let conn = self
            .pool
            .get()
            .map_err(|e| EasyError::new("Pool checkout failed for select_foreach", e))?;

        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| EasyError::new("Query preparation failed", e))?;

        let mut rows = stmt
            .query([])
            .map_err(|e| EasyError::new("Query execution failed", e))?;

        while let Some(row_result) = rows
            .next()
            .map_err(|e| EasyError::new("Iteration error", e))?
        {
            let dynamic_row = Self::row_to_dynamic(row_result)
                .map_err(|e| EasyError::new("Row conversion failed", e))?;
            f(dynamic_row)?;
        }
        Ok(())
    }

    pub fn execute(&self, sql: &str) -> Result<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| EasyError::new("Pool checkout failed for execute", e))?;

        conn.execute(sql, [])
            .map_err(|e| EasyError::new("SQL execution failed", e))?;
        Ok(())
    }

    /// Execute multiple SQL statements inside a single `BEGIN … COMMIT` transaction.
    ///
    /// All statements are sent to one connection in one round-trip, eliminating
    /// the per-statement pool-checkout + WAL-flush overhead. No-op when
    /// `statements` is empty.
    pub fn execute_many(&self, statements: &[String]) -> Result<()> {
        if statements.is_empty() {
            return Ok(());
        }
        let conn = self
            .pool
            .get()
            .map_err(|e| EasyError::new("Pool checkout failed for execute_many", e))?;
        let sql = format!("BEGIN;\n{};\nCOMMIT;", statements.join(";\n"));
        conn.execute_batch(&sql)
            .map_err(|e| EasyError::new("Batch transaction failed", e))?;
        Ok(())
    }

    pub fn sync(&self) -> Result<()> {
        self.execute("CHECKPOINT;")
    }
}
