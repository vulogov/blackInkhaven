extern crate log;

pub mod ai;
pub mod analysis;
pub mod common;
pub mod datastorage;
pub mod documentstorage;
pub mod embedding;
pub mod frequencytrackingstorage;
pub mod fts;
pub mod globals;
pub mod observability;
pub mod scheduler;
pub mod shard;
pub mod shardscache;
pub mod shardsinfo;
pub mod shardsmanager;
pub mod shardsmanager_aggregationsearch;
pub mod shardsmanager_docstore;
pub mod shardsmanager_drain;
pub mod shardsmanager_lsa_primary_textrank;
pub mod shardsmanager_ngram;
pub mod shardsmanager_primary_textrank;
pub mod shardsmanager_scripts;
pub mod shardsmanager_signals;
pub mod shardsmanager_templates_textrank;
pub mod shardsmanager_tplstorage;
pub mod storageengine;
pub mod vectorengine;
pub mod vm;
pub use analysis::{
    knn_summary, knn_summary_with, lsa_rank, lsa_summary, lsa_summary_with, ngram_anomaly,
    ngram_anomaly_with, ngram_remove_noise, ngram_remove_noise_with, textrank_rank,
    textrank_summary, textrank_summary_with, CausalCandidate, EventCluster, KnnConfig,
    LdaConfig, LsaConfig, NgramAnomalyConfig, NgramNoiseConfig, RcaConfig, RcaResult,
    RcaTemplatesConfig, RcaTemplatesResult, SamplePoint, TelemetryTrend,
    TemplateCausalCandidate, TemplateCluster, TextRankConfig, TopicSummary,
};
pub use common::generator::{Generator, LogFormat};
pub use common::jsonfingerprint::json_fingerprint;
pub use common::pipe;
pub use common::uuid::timestamp_from_v7;
pub use datastorage::{BlobStorage, JsonStorage, JsonStorageConfig};
pub use frequencytrackingstorage::FrequencyTracking;
pub use documentstorage::{results_to_strings, DocumentStorage};
pub use embedding::EmbeddingEngine;
pub use fts::FTSEngine;
pub use globals::{dbpath_from_config, get_db, init_db, sync_db};
pub use observability::{ObservabilityStorage, ObservabilityStorageConfig};
pub use shard::Shard;
pub use shardscache::ShardsCache;
pub use shardsinfo::ShardInfoEngine;
pub use shardsmanager::ShardsManager;
pub use storageengine::StorageEngine;
pub use vectorengine::VectorEngine;
pub use vm::context;
pub use scheduler::Scheduler;
pub use vm::workers::{submit_script, submit_script_with_id};
pub use vm::{bund_eval, init_adam};
pub mod setloglevel;

/// Version string of the embedded `bundcore` BUND VM crate.
///
/// Re-exported here so bdsweb / bdsnode / any other binary built on top
/// of bdslib can surface the BUND VM's version (in the footer, in
/// `v2/status`, etc.) without taking a direct dependency on `bundcore`.
pub fn bundcore_version() -> String {
    bundcore::version()
}
