//! 2.0 perf harness — `inkhaven _bench-embed` (hidden). Times the embedding
//! engine on N sample texts and reports throughput. This is the index-build /
//! embedding-throughput metric — the dominant cost of building the HNSW index —
//! isolated from store I/O. (`reindex` skips content-unchanged paragraphs, so it
//! can't measure embed cost directly.) The model-load (~470 ms) runs as a warm-up
//! and is excluded from the timed loop.

use crate::config::Config;
use crate::error::{Error, Result};

/// A small multilingual sample set so the throughput reflects the Unicode path.
const SAMPLES: &[&str] = &[
    "The harbor had been quiet since the last storm, the boats tucked tight in their slips.",
    "Елена остановилась на пороге, прислушиваясь к шагам внизу.",
    "Hélène s'arrêta sur le seuil, guettant le bruit des pas en contrebas.",
    "Das Morgenlicht fiel aufs Fenster und färbte den Raum bernsteinfarben.",
    "El puerto estaba tranquilo tras la última tormenta, las barcas bien amarradas.",
    "Marcus said nothing for a long moment, then nodded once and walked away.",
];

pub fn run(count: usize) -> Result<()> {
    // The configured model + the shared inkhaven model cache (so we embed with
    // exactly what production uses; no re-download).
    let cfg = Config::default();
    let engine = crate::store::build_embedding_engine(&cfg.embeddings.model)?;

    // Warm-up: the first embed loads the ONNX model (~470 ms) — excluded.
    engine.embed(SAMPLES[0]).map_err(|e| Error::Store(format!("embed warm-up: {e}")))?;

    let t = std::time::Instant::now();
    for i in 0..count.max(1) {
        engine
            .embed(SAMPLES[i % SAMPLES.len()])
            .map_err(|e| Error::Store(format!("embed: {e}")))?;
    }
    let total = t.elapsed();
    let per_sec = count as f64 / total.as_secs_f64().max(1e-9);

    println!("embed_count: {count}");
    println!("embed_total_us: {}", total.as_micros());
    println!("embed_per_sec: {per_sec:.1}");
    Ok(())
}
