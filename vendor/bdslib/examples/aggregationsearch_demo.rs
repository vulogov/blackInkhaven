/// ShardsManager::aggregationsearch() demo — unified telemetry + document search.
///
/// The scenario: a Kubernetes platform operations assistant.  Cluster telemetry
/// (pod restarts, OOM kills, node pressure events, HPA scaling, network errors)
/// arrives continuously through the shard layer.  Runbooks and post-mortems live
/// in the embedded DocumentStorage.  aggregationsearch() fires both a time-scoped
/// vector search over the shard store and a semantic search over the document store
/// in a single parallel call, returning results under "observability" and
/// "documents" respectively.
///
/// Sections:
///   1.  Construction      — config, ShardsManager::with_embedding, docstore path
///   2.  Document corpus   — 3 small runbooks (doc_add) + 2 large docs (doc_add_from_file)
///   3.  Telemetry corpus  — 4 phases × ~30 records: baseline → pressure → incident → recovery
///   4.  aggregationsearch — 4 queries covering distinct failure modes; inspect both sides
///   5.  Duration scoping  — same query at 1h vs 6h window shows time-bounded telemetry
///                           while documents remain invariant to the window
///   6.  Result structure  — enumerate fields present in observability and document hits
use bdslib::common::error::{err_msg, Result};
use bdslib::embedding::Model;
use bdslib::shardsmanager::ShardsManager;
use bdslib::EmbeddingEngine;
use serde_json::{json, Value as JsonValue};
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};
use tempfile::TempDir;

// ── small runbook documents (stored as single records via doc_add) ─────────────

const RUNBOOK_CRASHLOOP: &str = "\
Pod CrashLoopBackOff Runbook

A pod in CrashLoopBackOff is restarting repeatedly because its main container \
exits with a non-zero code or is OOM-killed before the liveness probe window \
expires. Kubernetes backs off exponentially: 10 s, 20 s, 40 s up to 5 minutes.

Immediate triage: kubectl describe pod <name> -n <namespace> — check the Last \
State section for exit code and reason. Exit 137 is an OOM kill. Exit 1 usually \
indicates a configuration or dependency error. Check the previous container logs \
with kubectl logs <pod> --previous to read the final stderr before the crash.

Common causes and fixes: missing ConfigMap or Secret referenced in the pod spec \
→ verify the resource exists and the key names match exactly. OOM limit too low \
→ increase resources.limits.memory in the Deployment or check for a memory leak. \
Readiness probe misconfigured → the probe fails before the application is ready, \
causing eviction. Init container failure → check init container logs separately. \
Check the cluster event stream for scheduling failures or node-level issues that \
may force the pod onto a resource-starved node.";

const RUNBOOK_NODE_PRESSURE: &str = "\
Node Memory Pressure Response

Node memory pressure status is set when the kubelet detects available memory \
below the hard eviction threshold (default 100 MiB free). Pods on a pressured \
node may be evicted in order of their QoS class: BestEffort first, then \
Burstable, then Guaranteed.

Immediate triage: kubectl describe node <name> — check Conditions for \
MemoryPressure=True and the Allocatable versus Requests/Limits table. Run \
kubectl top node to see real-time memory usage. Identify the heaviest pods with \
kubectl top pods --all-namespaces --sort-by=memory.

Mitigation: cordon the pressured node with kubectl cordon <node> to prevent \
new pod scheduling. Evict the top memory consumer if it is not in a critical \
namespace. If the node is chronically over-committed add a memory limit range \
to the namespace LimitRange object. If this is happening across multiple nodes \
simultaneously, check whether a recent Deployment rollout increased memory \
footprint — roll it back and tighten the resource requests.";

const RUNBOOK_NETWORK: &str = "\
Pod Network Connectivity Debugging Guide

Symptoms of a network issue: pod-to-pod connection refused, DNS resolution \
failures, service endpoint unreachable, intermittent connection resets.

Diagnosis steps: verify that the target pod is Running and Ready with kubectl \
get pods. Check service endpoints with kubectl get endpoints <service-name>; an \
empty Endpoints list means no pods matched the selector. Confirm the NetworkPolicy \
allows ingress from the source namespace and pod labels — a restrictive default-deny \
policy is the most common cause of unexpected connection refusals.

DNS issues: run kubectl exec -it <pod> -- nslookup kubernetes.default to \
confirm cluster DNS is responding. If lookup fails, check CoreDNS pod status in \
kube-system. For latency spikes check the ndots setting in /etc/resolv.conf — \
a high ndots value (default 5) causes multiple unnecessary search-domain retries \
before resolving short names.

MTU mismatches between the overlay network and the underlying infrastructure \
cause intermittent packet loss under load. Check the CNI plugin version against \
the node kernel version if you see TCP retransmits or application-layer timeouts \
without any obvious error messages.";

// ── large documents stored with doc_add_from_file (chunked automatically) ─────

const CLUSTER_INCIDENT_RUNBOOK: &str = "\
Kubernetes Cluster Major Incident Runbook

This runbook applies to P1 and P2 incidents affecting cluster-wide availability \
including mass pod evictions, control plane degradation, widespread OOM events, \
and networking outages spanning multiple namespaces.

Phase 1: Immediate Assessment (0 to 5 minutes)
Confirm the blast radius by running kubectl get nodes — are nodes NotReady or \
under pressure? Check the control plane health with kubectl get componentstatuses \
and verify API server response times. If the API server is slow or unresponsive \
escalate to the infrastructure team immediately. Note whether the incident \
correlates with a recent deployment, a cron job firing, or an autoscaler event.

Review cluster-wide events with kubectl get events --all-namespaces --sort-by=\
.metadata.creationTimestamp | tail -40. Look for patterns: are evictions \
concentrated on a single node, a single namespace, or spread across the cluster? \
Check the HPA status for all deployments that are under pressure — an aggressive \
scale-up can exhaust node memory faster than the cluster autoscaler can provision \
new capacity.

Phase 2: Triage by Failure Mode (5 to 20 minutes)
OOM or memory pressure scenario: identify the namespace and workload driving \
memory growth. Apply an emergency memory limit patch with kubectl patch deployment \
to cap the offending workload. If evictions continue, cordon the most pressured \
node to drain it. Do not delete pods with kubectl delete unless you understand \
their PodDisruptionBudget — you may drop below the minimum available replica count.

Network outage scenario: check CNI plugin pod health in kube-system. A crashed \
flannel or calico pod will prevent new pod networking from being configured. \
Restart the CNI pods if they are in a crash loop but do not restart more than one \
at a time. Verify that node IP routes are intact with ip route on the affected \
node. If a NetworkPolicy change was recently applied, roll it back and check \
whether connectivity restores.

CrashLoopBackOff epidemic scenario: an epidemic — many pods across namespaces \
crashing simultaneously — usually indicates a shared dependency failure such as \
a broken ConfigMap rollout, a certificate expiry, or a database password rotation \
that was not propagated to all services. Run kubectl get pods --all-namespaces \
--field-selector=status.phase=Running | grep -v Running to enumerate unhealthy \
pods across all namespaces.

Phase 3: Recovery and Verification (20 to 45 minutes)
After applying the mitigation, monitor the rolling recovery. Uncordon nodes only \
after memory pressure clears. Verify that HPA targets have stabilised and that \
the error rate on all affected services is below their respective SLO thresholds. \
Run the smoke test suite for each namespace that experienced evictions. Confirm \
that persistent volumes are still attached and healthy after any node drain.

Phase 4: Post-Incident
Document the incident with a precise timeline, total affected pods and namespaces, \
blast radius in terms of customer impact, root cause, and corrective actions. \
Assign ownership for each follow-up item with a deadline. Review whether the \
Cluster Autoscaler scale-out speed was sufficient — if nodes were unavailable for \
more than 10 minutes, consider adjusting scale-up cooldown or pre-provisioning \
buffer capacity for the next peak window.";

const OOM_POSTMORTEM: &str = "\
Post-Mortem: Cluster-Wide OOM Incident — Data Processing Namespace

Executive Summary
A batch data-processing job in the analytics namespace consumed all available \
memory on three of eight nodes, triggering mass evictions across co-located \
workloads. The incident lasted 38 minutes and affected the payment service, \
the notification service, and the user-data API. Root cause was an unbounded \
memory allocation in a new Spark job introduced without a resource limit.

Timeline
T minus 30 minutes: Spark job analytics/etl-nightly-reprocess submitted by \
CI pipeline. The job spec omitted resources.limits.memory (a required field in \
other namespaces but not enforced in analytics by LimitRange policy at the time).

T plus 0 minutes: node-01 memory pressure alert fires. kubelet begins evicting \
BestEffort pods from node-01. Payment service pods are BestEffort (no resource \
requests set) and are evicted first.

T plus 6 minutes: OOM killer fires on node-02 and node-03. The Spark executor \
processes grow to consume over 14 GiB each on a 16 GiB node. The eviction \
cascade reaches Burstable pods including the notification service.

T plus 12 minutes: circuit breaker opens on payment service due to pod \
unavailability. On-call engineer is paged. The Spark job is identified as the \
cause from kubectl top pods output.

T plus 18 minutes: Spark job cancelled with kubectl delete job. Memory usage \
begins to drop. Evicted pods restart and pass readiness probes within 4 minutes \
of cancellation.

T plus 38 minutes: all services report healthy. Circuit breaker closes. \
Incident resolved.

Root Cause
Absence of resource limits in the analytics namespace combined with no admission \
webhook to reject limit-free workloads. A single runaway job was able to consume \
all node memory, causing evictions that cascaded into production namespaces.

Corrective Actions
Add a LimitRange object to the analytics namespace enforcing a 4 GiB maximum \
memory limit per container. Deploy an OPA Gatekeeper policy to reject any \
Deployment or Job spec that omits resource limits cluster-wide. Add a dedicated \
node pool for batch workloads using node taints and tolerations to prevent \
batch jobs from landing on nodes hosting production services. Add alerting on \
the ratio of namespace memory requests to node allocatable memory — trigger at \
80 percent to give the autoscaler time to provision before pressure starts.";

// ── helpers ───────────────────────────────────────────────────────────────────

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn aligned_hour(t: u64) -> u64 {
    (t / 3600) * 3600
}

fn hr() {
    println!("════════════════════════════════════════════════════════════════");
}

fn sep() {
    println!("────────────────────────────────────────────────────────────────");
}

fn preview(s: &str, n: usize) -> String {
    let s = s.trim();
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n).collect::<String>())
    }
}

fn show_obs_hit(hit: &JsonValue) {
    let key   = hit["key"].as_str().unwrap_or("?");
    let ts    = hit["timestamp"].as_u64().unwrap_or(0);
    let score = hit.get("_score").and_then(|v| v.as_f64()).unwrap_or(0.0);
    let data  = hit["data"].as_str()
        .map(|s| format!("\"{s}\""))
        .unwrap_or_else(|| preview(&hit["data"].to_string(), 60));
    println!("    [{score:.4}]  key={key:<22}  ts={ts}  data={}", preview(&data, 62));
}

fn show_doc_hit(hit: &JsonValue) {
    let score     = hit["score"].as_f64().unwrap_or(0.0);
    let name      = hit["metadata"]["document_name"]
        .as_str()
        .or_else(|| hit["metadata"]["name"].as_str())
        .unwrap_or("?");
    let chunk_idx = hit["metadata"]["chunk_index"].as_u64();
    let n_chunks  = hit["metadata"]["n_chunks"].as_u64();
    let content   = hit["document"].as_str().unwrap_or("");
    match (chunk_idx, n_chunks) {
        (Some(ci), Some(nc)) =>
            println!("    [{score:.3}]  chunk [{ci}/{nc}]  {name}"),
        _ =>
            println!("    [{score:.3}]  {name}"),
    }
    println!("           \"{}\"", preview(content, 88));
}

fn show_aggregation(result: &JsonValue, obs_limit: usize, doc_limit: usize) {
    let obs  = result["observability"].as_array().unwrap();
    let docs = result["documents"].as_array().unwrap();
    println!("  observability: {} hit(s)", obs.len());
    for hit in obs.iter().take(obs_limit) { show_obs_hit(hit); }
    if obs.len() > obs_limit {
        println!("    … {} more", obs.len() - obs_limit);
    }
    println!("  documents: {} hit(s)", docs.len());
    for hit in docs.iter().take(doc_limit) { show_doc_hit(hit); }
    if docs.len() > doc_limit {
        println!("    … {} more", docs.len() - doc_limit);
    }
}

// ── telemetry record generation ───────────────────────────────────────────────

struct Phase {
    label:   &'static str,
    base_ts: u64,
    metrics: &'static [(&'static str, u64)],
    errors:  &'static [&'static str],
    warns:   &'static [&'static str],
    infos:   &'static [&'static str],
    events:  &'static [(&'static str, &'static str)],
}

fn generate_phase(p: &Phase) -> Vec<JsonValue> {
    let mut records = Vec::new();
    let mut ts = p.base_ts;

    macro_rules! push {
        ($key:expr, $data:expr) => {{
            ts += 60;
            records.push(json!({"timestamp": ts, "key": $key, "data": $data}));
        }};
    }

    for (key, val) in p.metrics {
        for offset in 0u64..5 {
            push!(*key, val + (offset * 3) % 13);
        }
    }
    for msg in p.errors { push!("log.error", *msg); }
    for msg in p.warns  { push!("log.warn",  *msg); }
    for msg in p.infos  { push!("log.info",  *msg); }
    for (pod, event) in p.events {
        push!("k8s.event", json!({"pod": pod, "event": event}));
    }
    records
}

fn build_phases(now: u64) -> Vec<Phase> {
    vec![
        Phase {
            label:   "baseline",
            base_ts: aligned_hour(now - 4 * 3600),
            metrics: &[
                ("k8s.cpu_pct",  32), ("k8s.mem_pct", 41),
                ("k8s.pod_restarts", 0), ("k8s.net_rx_mb", 120),
            ],
            errors: &[],
            warns:  &[
                "etcd latency elevated at 180ms on leader node",
                "HPA unable to scale analytics-worker: no metrics available",
            ],
            infos:  &[
                "cluster autoscaler added node pool-node-07 to analytics pool",
                "nightly etl job submitted to analytics namespace",
                "CoreDNS scaled up from 2 to 3 replicas by HPA",
            ],
            events: &[
                ("analytics/etl-nightly-0", "Scheduled"),
                ("analytics/etl-nightly-0", "Pulled"),
                ("analytics/etl-nightly-0", "Started"),
            ],
        },
        Phase {
            label:   "pressure",
            base_ts: aligned_hour(now - 3 * 3600),
            metrics: &[
                ("k8s.cpu_pct", 71), ("k8s.mem_pct", 83),
                ("k8s.pod_restarts", 3), ("k8s.net_rx_mb", 340),
            ],
            errors: &[
                "node-01 memory pressure threshold exceeded kubelet evicting pods",
                "pod payment/payment-api-7d4f9b-xkvp2 evicted due to memory pressure",
                "pod payment/payment-api-7d4f9b-rmq7t evicted due to memory pressure",
            ],
            warns:  &[
                "node-01 available memory below soft eviction threshold",
                "analytics/etl-nightly-0 memory usage 11.2 GiB and growing",
                "HPA scaled payment-api from 4 to 7 replicas to compensate for evictions",
                "replication lag on standby-db now 15 seconds",
            ],
            infos:  &[
                "kubelet eviction manager started on node-01",
                "payment-api pods rescheduled to node-04 and node-05",
            ],
            events: &[
                ("payment/payment-api-7d4f9b-xkvp2", "Evicted"),
                ("payment/payment-api-7d4f9b-rmq7t", "Evicted"),
                ("analytics/etl-nightly-0", "OOMKilled"),
            ],
        },
        Phase {
            label:   "incident",
            base_ts: aligned_hour(now - 2 * 3600),
            metrics: &[
                ("k8s.cpu_pct", 92), ("k8s.mem_pct", 96),
                ("k8s.pod_restarts", 18), ("k8s.net_rx_mb", 28),
            ],
            errors: &[
                "OOM killer fired on node-02 process analytics-etl terminated",
                "OOM killer fired on node-03 process analytics-etl terminated",
                "pod notification/notif-worker-84pqr CrashLoopBackOff exit code 137",
                "pod notification/notif-worker-52tgx CrashLoopBackOff exit code 137",
                "pod userdata/user-api-6bfr9 CrashLoopBackOff exit code 137",
                "circuit breaker opened on payment-service all downstream calls rejected",
                "network timeout connecting to user-data-db from payment-api replica 3",
                "DNS resolution failure for user-data-svc.default.svc.cluster.local",
                "connection refused payment-api to notification-service port 8080",
            ],
            warns:  &[
                "node-02 and node-03 NotReady kubelet not posting node status",
                "NetworkPolicy blocking ingress to notification namespace from payment",
                "TCP retransmit rate on eth0 node-02 exceeded 8 percent",
            ],
            infos:  &[
                "P1 incident declared cluster-wide OOM cascade in progress",
                "on-call engineer paged via PagerDuty",
                "Spark job analytics/etl-nightly-0 identified as root cause",
            ],
            events: &[
                ("notification/notif-worker-84pqr", "BackOff"),
                ("userdata/user-api-6bfr9", "BackOff"),
                ("payment/payment-api-5cxv8", "FailedMount"),
            ],
        },
        Phase {
            label:   "recovery",
            base_ts: aligned_hour(now - 1 * 3600),
            metrics: &[
                ("k8s.cpu_pct", 44), ("k8s.mem_pct", 52),
                ("k8s.pod_restarts", 1), ("k8s.net_rx_mb", 130),
            ],
            errors: &[],
            warns:  &[
                "node-02 still draining before uncordon",
                "HPA for payment-api scaling back down from 7 to 4 replicas",
            ],
            infos:  &[
                "Spark job cancelled memory usage dropping on all nodes",
                "node-02 and node-03 returned to Ready state",
                "CrashLoopBackOff pods in notification and userdata namespaces recovering",
                "circuit breaker closed payment-service traffic resuming",
                "DNS resolution restored for all services in default namespace",
                "network connectivity restored payment-api to notification-service",
                "replication lag back to normal 0.3 seconds on standby-db",
                "incident resolved all services nominal",
            ],
            events: &[
                ("analytics/etl-nightly-0", "Killed"),
                ("notification/notif-worker-84pqr", "Started"),
                ("userdata/user-api-6bfr9", "Started"),
            ],
        },
    ]
}

// ── main ──────────────────────────────────────────────────────────────────────

fn main() -> Result<()> {
    println!("Loading AllMiniLML6V2 embedding model…");
    let embedding = EmbeddingEngine::new(Model::AllMiniLML6V2, None)
        .map_err(|e| err_msg(format!("embedding init: {e}")))?;
    println!("Model ready.\n");

    let root_dir = TempDir::new().map_err(|e| err_msg(e.to_string()))?;
    let doc_dir  = TempDir::new().map_err(|e| err_msg(e.to_string()))?;
    let root     = root_dir.path().to_str().unwrap();

    // ── Section 1: Construction ───────────────────────────────────────────────

    hr();
    println!(" Section 1: Construction");
    hr();

    let config_path = format!("{root}/manager.hjson");
    let db_path     = format!("{root}/db");
    let hjson = format!(
        "{{\n  dbpath: \"{db_path}\"\n  shard_duration: \"1h\"\n  pool_size: 4\n}}\n"
    );
    fs::write(&config_path, &hjson).map_err(|e| err_msg(e.to_string()))?;

    let mgr = ShardsManager::with_embedding(&config_path, embedding)?;

    println!();
    println!("  ShardsManager ready");
    println!("  dbpath:          {db_path}");
    println!("  shard_duration:  1h  (each phase lands in its own shard)");
    println!("  docstore path:   {db_path}/docstore");
    println!("  embedding model: AllMiniLML6V2 (shared — shard + docstore use same Arc)");
    println!();

    // ── Section 2: Document corpus ────────────────────────────────────────────

    hr();
    println!(" Section 2: Document corpus");
    hr();
    println!();

    // Small runbooks (single-record, no chunking)
    let id_crashloop = mgr.doc_add(
        json!({"name": "Pod CrashLoopBackOff Runbook",
               "category": "runbook", "domain": "kubernetes", "severity": "P2"}),
        RUNBOOK_CRASHLOOP.as_bytes(),
    )?;
    println!("  doc_add: Pod CrashLoopBackOff Runbook        → {id_crashloop}");

    let id_pressure = mgr.doc_add(
        json!({"name": "Node Memory Pressure Response",
               "category": "runbook", "domain": "kubernetes", "severity": "P1"}),
        RUNBOOK_NODE_PRESSURE.as_bytes(),
    )?;
    println!("  doc_add: Node Memory Pressure Response       → {id_pressure}");

    let id_network = mgr.doc_add(
        json!({"name": "Pod Network Connectivity Debugging",
               "category": "runbook", "domain": "kubernetes", "severity": "P2"}),
        RUNBOOK_NETWORK.as_bytes(),
    )?;
    println!("  doc_add: Pod Network Connectivity Debugging  → {id_network}");

    // Large documents (chunked)
    let cluster_path = doc_dir.path().join("cluster_incident_runbook.txt");
    let oom_path     = doc_dir.path().join("oom_postmortem.txt");
    fs::write(&cluster_path, CLUSTER_INCIDENT_RUNBOOK).map_err(|e| err_msg(e.to_string()))?;
    fs::write(&oom_path,     OOM_POSTMORTEM).map_err(|e| err_msg(e.to_string()))?;

    let id_cluster_rb = mgr.doc_add_from_file(
        cluster_path.to_str().unwrap(),
        "Kubernetes Cluster Major Incident Runbook",
        240, 18.0,
    )?;
    let cluster_rb_meta = mgr.doc_get_metadata(id_cluster_rb)?.unwrap();
    println!("  doc_add_from_file: Cluster Major Incident Runbook");
    println!("    doc_id={id_cluster_rb}  n_chunks={}  slice=240  overlap=18%",
        cluster_rb_meta["n_chunks"]);

    let id_oom_pm = mgr.doc_add_from_file(
        oom_path.to_str().unwrap(),
        "Container OOM Post-Mortem",
        230, 20.0,
    )?;
    let oom_pm_meta = mgr.doc_get_metadata(id_oom_pm)?.unwrap();
    println!("  doc_add_from_file: Container OOM Post-Mortem");
    println!("    doc_id={id_oom_pm}  n_chunks={}  slice=230  overlap=20%",
        oom_pm_meta["n_chunks"]);

    let total_docs = 3 + cluster_rb_meta["n_chunks"].as_u64().unwrap_or(0)
                       + oom_pm_meta["n_chunks"].as_u64().unwrap_or(0) + 2;
    println!();
    println!("  Document store: 5 source documents  ({total_docs} total indexed records)");
    println!("    3 small runbooks (1 HNSW entry each)");
    println!("    2 large docs → chunk records + 1 document-level record each");

    // ── Section 3: Telemetry corpus ───────────────────────────────────────────

    println!();
    hr();
    println!(" Section 3: Telemetry corpus — 4 incident phases");
    hr();
    println!();

    let now    = now_secs();
    let phases = build_phases(now);
    let mut total_tel = 0usize;

    for phase in &phases {
        let docs  = generate_phase(phase);
        let count = docs.len();
        mgr.add_batch(docs)?;
        total_tel += count;
        println!(
            "  phase={:<12}  base_ts={}  records={}  shards={}",
            phase.label, phase.base_ts, count, mgr.cache().cached_count()
        );
    }
    println!("\n  Total telemetry records: {total_tel}  across {} shards",
        mgr.cache().cached_count());

    // ── Section 4: aggregationsearch ─────────────────────────────────────────

    println!();
    hr();
    println!(" Section 4: ShardsManager::aggregationsearch()");
    println!(" Parallel vector search over telemetry shards + semantic doc search");
    hr();

    // ── Query 4a: CrashLoopBackOff ────────────────────────────────────────────

    println!();
    sep();
    let q_crash = "pod CrashLoopBackOff container exit restart backoff";
    println!("  Query A: \"{q_crash}\"");
    sep();
    let r_crash = mgr.aggregationsearch("6h", q_crash)?;
    show_aggregation(&r_crash, 5, 3);

    // ── Query 4b: OOM / memory pressure ──────────────────────────────────────

    println!();
    sep();
    let q_oom = "OOM killer memory pressure eviction node kubelet";
    println!("  Query B: \"{q_oom}\"");
    sep();
    let r_oom = mgr.aggregationsearch("6h", q_oom)?;
    show_aggregation(&r_oom, 5, 3);

    // ── Query 4c: Network / DNS / connection refused ──────────────────────────

    println!();
    sep();
    let q_net = "network timeout DNS resolution failure connection refused TCP";
    println!("  Query C: \"{q_net}\"");
    sep();
    let r_net = mgr.aggregationsearch("6h", q_net)?;
    show_aggregation(&r_net, 5, 3);

    // ── Query 4d: Circuit breaker recovery ───────────────────────────────────

    println!();
    sep();
    let q_cb = "circuit breaker payment service recovered closed traffic resumed";
    println!("  Query D: \"{q_cb}\"");
    sep();
    let r_cb = mgr.aggregationsearch("6h", q_cb)?;
    show_aggregation(&r_cb, 5, 3);

    // ── Section 5: Duration scoping ───────────────────────────────────────────

    println!();
    hr();
    println!(" Section 5: Duration scoping — same query, different lookback windows");
    println!(" Observability results are time-bounded; documents are global.");
    hr();

    let q_scope = "OOM kill pod eviction node memory";

    for window in &["1h", "2h", "4h", "6h"] {
        let r = mgr.aggregationsearch(window, q_scope)?;
        let obs_n = r["observability"].as_array().unwrap().len();
        let doc_n = r["documents"].as_array().unwrap().len();
        println!("  window={window:<4}  observability={obs_n:<3} hit(s)  documents={doc_n} hit(s)");
    }

    println!();
    println!("  Observation: document count is constant across all windows —");
    println!("  the document store has no time dimension. Telemetry hit count");
    println!("  grows with the window as more shards fall within the lookback range.");

    // ── Section 6: Result structure ───────────────────────────────────────────

    println!();
    hr();
    println!(" Section 6: Result structure — fields present in each hit type");
    hr();

    let r_struct = mgr.aggregationsearch("6h", "pod crash restart OOM memory")?;

    let obs_hits  = r_struct["observability"].as_array().unwrap();
    let doc_hits  = r_struct["documents"].as_array().unwrap();

    // Observability hit fields
    println!();
    println!("  observability[0] fields:");
    if let Some(hit) = obs_hits.first() {
        for key in ["id", "timestamp", "key", "data", "_score", "secondaries"] {
            let present = hit.get(key).map(|v| !v.is_null()).unwrap_or(false);
            let marker  = if present { "✓" } else { "✗" };
            let snippet = hit.get(key)
                .map(|v| preview(&v.to_string(), 50))
                .unwrap_or_default();
            println!("    {marker} {key:<14}  {snippet}");
        }
    } else {
        println!("    (no observability hits — widen the window)");
    }

    // Document hit fields — find one chunked and one whole-doc hit
    println!();
    println!("  documents — chunked hit fields:");
    let chunk_hit = doc_hits.iter().find(|h| h["metadata"]["chunk_index"].is_number());
    if let Some(hit) = chunk_hit {
        for key in ["id", "score", "document"] {
            let present = hit.get(key).map(|v| !v.is_null()).unwrap_or(false);
            let marker  = if present { "✓" } else { "✗" };
            let snippet = hit.get(key)
                .map(|v| v.as_str()
                    .map(|s| preview(s, 50))
                    .unwrap_or_else(|| preview(&v.to_string(), 50)))
                .unwrap_or_default();
            println!("    {marker} {key:<14}  {snippet}");
        }
        println!("  documents — chunked hit metadata fields:");
        let meta = &hit["metadata"];
        for mkey in ["document_name", "document_id", "chunk_index", "n_chunks"] {
            let present = meta.get(mkey).map(|v| !v.is_null()).unwrap_or(false);
            let marker  = if present { "✓" } else { "✗" };
            let snippet = meta.get(mkey)
                .map(|v| preview(&v.to_string(), 50))
                .unwrap_or_default();
            println!("    {marker} metadata.{mkey:<12}  {snippet}");
        }
    } else {
        println!("    (no chunked doc hits for this query)");
    }

    println!();
    println!("  documents — whole-doc hit metadata fields:");
    let whole_hit = doc_hits.iter().find(|h| !h["metadata"]["chunk_index"].is_number());
    if let Some(hit) = whole_hit {
        let meta = &hit["metadata"];
        for mkey in ["name", "category", "domain", "severity"] {
            let present = meta.get(mkey).map(|v| !v.is_null()).unwrap_or(false);
            let marker  = if present { "✓" } else { "✗" };
            let snippet = meta.get(mkey)
                .map(|v| preview(&v.to_string(), 50))
                .unwrap_or_default();
            println!("    {marker} metadata.{mkey:<12}  {snippet}");
        }
    } else {
        println!("    (no whole-doc hits for this query)");
    }

    // ── Summary ───────────────────────────────────────────────────────────────

    println!();
    hr();
    println!(" Done.");
    println!("  Telemetry:  {total_tel} records across {} shards (1-hour buckets)",
        mgr.cache().cached_count());
    println!("  Docstore:   5 source documents  ({total_docs} indexed records)");
    println!("  aggregationsearch queries: 4 × 6h + 4 × duration-scope variants");
    println!("  Both searches run in parallel via rayon::join;");
    println!("  results keyed as \"observability\" (vector-ranked) + \"documents\" (semantic).");
    hr();

    Ok(())
}
