use bdslib::common::error::{err_msg, Result};
use bdslib::StorageEngine;

const SCHEMA: &str = "
    CREATE TABLE events (
        id      INTEGER PRIMARY KEY,
        source  TEXT    NOT NULL,
        level   TEXT    NOT NULL,
        message TEXT    NOT NULL,
        score   DOUBLE
    );
";

fn cast_err(e: Box<dyn std::error::Error>) -> bdslib::common::error::Error {
    err_msg(e.to_string())
}

fn main() -> Result<()> {
    let engine = StorageEngine::new(":memory:", SCHEMA, 4)?;
    println!("StorageEngine ready (in-memory)");

    // --- insert ---
    let rows = [
        (1, "auth",    "INFO",  "user alice logged in",       0.95),
        (2, "auth",    "WARN",  "failed login for bob",       0.40),
        (3, "storage", "ERROR", "disk quota exceeded",        0.10),
        (4, "network", "INFO",  "connection established",     0.88),
        (5, "auth",    "INFO",  "user charlie logged in",     0.92),
    ];

    for (id, src, level, msg, score) in rows {
        engine.execute(&format!(
            "INSERT INTO events VALUES ({id}, '{src}', '{level}', '{msg}', {score})"
        ))?;
    }
    println!("Inserted {} events", rows.len());

    // --- select_all: fetch specific columns ---
    println!("\n-- High-confidence events (score > 0.8) --");
    let results = engine.select_all(
        "SELECT id, source, message, score FROM events WHERE score > 0.8 ORDER BY score DESC",
    )?;
    for row in &results {
        let id     = row[0].cast_int().map_err(cast_err)?;
        let source = row[1].cast_string().map_err(cast_err)?;
        let msg    = row[2].cast_string().map_err(cast_err)?;
        let score  = row[3].cast_float().map_err(cast_err)?;
        println!("  [{id}] ({source}) {msg}  score={score:.2}");
    }

    // --- select_foreach: stream rows without collecting ---
    println!("\n-- Auth events streamed via select_foreach --");
    let mut count = 0usize;
    engine.select_foreach(
        "SELECT level, message FROM events WHERE source = 'auth' ORDER BY id",
        |row| {
            let level = row[0].cast_string().unwrap();
            let msg   = row[1].cast_string().unwrap();
            println!("  [{level}] {msg}");
            count += 1;
            Ok(())
        },
    )?;
    println!("  ({count} auth events)");

    // --- aggregate ---
    let agg = engine.select_all(
        "SELECT level, count(*) FROM events GROUP BY level ORDER BY level",
    )?;
    println!("\n-- Event counts by level --");
    for row in &agg {
        let level = row[0].cast_string().map_err(cast_err)?;
        let n     = row[1].cast_int().map_err(cast_err)?;
        println!("  {level}: {n}");
    }

    // --- update + verify ---
    engine.execute("UPDATE events SET score = 0.0 WHERE level = 'ERROR'")?;
    let error_rows = engine.select_all(
        "SELECT message, score FROM events WHERE level = 'ERROR'",
    )?;
    println!("\n-- ERROR events after score reset --");
    for row in &error_rows {
        let msg   = row[0].cast_string().map_err(cast_err)?;
        let score = row[1].cast_float().map_err(cast_err)?;
        println!("  {msg} -> score={score}");
    }

    // --- sync (checkpoint; no-op for in-memory but shows the API) ---
    engine.sync()?;
    println!("\nCheckpoint complete.");

    Ok(())
}
