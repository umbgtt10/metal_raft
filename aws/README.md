# MetalRaft AWS Deployment & Observability

This document outlines the strategy for deploying MetalRaft on AWS and implementing comprehensive observability.

## Table of Contents

- [AWS Deployment Strategy](#aws-deployment-strategy)
- [Observability Strategy](#observability-strategy)
- [Implementation Plan](#implementation-plan)
- [Quick Wins](#quick-wins)
- [Recommendations](#recommendations)

---

## AWS Deployment Strategy

### Architecture Options

#### Option 1: ECS Fargate (Recommended for MVP)

**Advantages:**
- Serverless container orchestration - no EC2 instance management
- Easy horizontal scaling with service auto-scaling
- Pay-per-use pricing (~$0.04/vCPU-hour)
- Excellent for 3-5 node Raft clusters
- Native integration with AWS service discovery (Cloud Map)
- Minimal operational overhead

**Rust Compatibility:**
- Compile to static binary with musl target
- Small container footprint (~10-20MB)
- Fast cold start times
- Efficient resource utilization

**Networking:**
- AWS Cloud Map for service discovery
- Application Load Balancer (ALB) for client HTTP requests
- Network Load Balancer (NLB) for low-latency linearizable reads
- Private VPC subnets for inter-node Raft communication

**Cost Estimate (3-node cluster):**
- Fargate: ~$30-50/month (0.25 vCPU, 0.5GB per node)
- Load Balancer: ~$16/month
- Storage (EBS via ECS volumes): ~$10/month
- Total: ~$60-80/month

#### Option 2: EKS (For Larger Deployments)

**Advantages:**
- Kubernetes StatefulSets provide stable network identities
- Persistent volume claims for Raft log storage
- Advanced orchestration (auto-healing, rolling updates)
- Multi-region federation support
- Large ecosystem of tooling (Helm, Operators)

**Best For:**
- Clusters with 5+ nodes
- Multi-region deployments
- Organizations already using Kubernetes
- Complex orchestration requirements

**Considerations:**
- Higher cost (~$73/month for control plane + worker nodes)
- Additional operational complexity
- Longer learning curve

**Cost Estimate (3-node cluster):**
- EKS control plane: $73/month
- Worker nodes (t3.medium): ~$75/month
- Storage (EBS): ~$10/month
- Total: ~$160/month

#### Option 3: EC2 with Auto Scaling Groups

**Advantages:**
- Maximum control over instance configuration
- Lower cost at scale
- Custom instance types (Graviton2/3 for Arm)
- Ability to use spot instances for cost optimization
- Direct EBS volume attachment

**Considerations:**
- Higher operational overhead (patching, monitoring)
- Manual service discovery configuration
- Requires more infrastructure automation

**Best For:**
- Cost-sensitive deployments at scale
- Need for specific hardware configurations
- Teams with strong EC2 operational experience

---

### Key AWS Services

#### Storage Layer

**Amazon EBS (Elastic Block Store) - gp3 volumes:**
- **Use Case**: Primary persistent storage for Raft logs and metadata
- **Performance**: 3,000 IOPS baseline, 125 MB/s throughput
- **Durability**: 99.8-99.9% annual failure rate
- **Features**: Encryption at rest, snapshots for backup
- **Cost**: $0.08/GB-month
- **Recommendation**: 20-50GB per node, sufficient for most workloads

**Amazon EFS (Elastic File System):**
- **Use Case**: Shared snapshot storage across nodes (advanced use case)
- **Features**: Multi-AZ, automatic scaling
- **Cost**: Higher ($0.30/GB-month)
- **Note**: Probably overkill for most Raft deployments

**Amazon S3:**
- **Use Case**:
  - Long-term snapshot archival
  - Disaster recovery backups
  - Cross-region snapshot replication
- **Features**: Glacier for cost-effective cold storage
- **Cost**: $0.023/GB-month (Standard), $0.004/GB-month (Glacier)
- **Strategy**: Archive snapshots older than 7 days

#### Networking

**VPC (Virtual Private Cloud):**
- **Configuration**:
  - Private subnets across 3 availability zones
  - NAT Gateway for outbound internet (updates, telemetry)
  - No direct internet ingress to Raft nodes
- **Security Groups**: Restrictive rules allowing only Raft protocol ports

**Network Load Balancer (NLB):**
- **Use Case**: Client-facing endpoint for linearizable reads
- **Features**:
  - Ultra-low latency (<1ms overhead)
  - Preserves source IP
  - Layer 4 load balancing
  - Health checks with auto-failover
- **Cost**: $0.0225/hour + $0.006/GB processed

**Application Load Balancer (ALB):**
- **Use Case**: HTTP/gRPC client requests with path-based routing
- **Features**:
  - TLS termination
  - Request routing
  - Integration with AWS WAF
- **Cost**: Similar to NLB

**AWS Cloud Map:**
- **Use Case**: Service discovery for Raft node-to-node communication
- **Features**:
  - DNS-based service discovery
  - Health checking
  - API-based registration/deregistration
- **Alternative**: Consul on ECS for more advanced service mesh

**AWS PrivateLink:**
- **Use Case**: Secure integration with other AWS services
- **Example**: Private connection to application services without internet traversal

#### High Availability

**Multi-AZ Deployment:**
- **Strategy**: Distribute Raft nodes across 3 availability zones
- **Benefits**:
  - Survives single AZ failure
  - Maintains quorum (3 nodes → 2 survive in 2 AZs)
  - Reduced blast radius
- **Consideration**: Inter-AZ latency (~1-2ms), acceptable for consensus

**Route53 Health Checks:**
- **Use Case**: Automatic DNS failover for client endpoints
- **Configuration**:
  - Health check every 30 seconds
  - Failover to healthy endpoint within 1 minute
  - Multi-region failover support

**Backup and Disaster Recovery:**
- **Strategy**:
  - Automated EBS snapshots every 6 hours
  - S3 snapshot replication to secondary region
  - Point-in-time recovery capability
  - RTO: <15 minutes, RPO: <6 hours

---

## Observability Strategy

### Metrics Layer (Prometheus + Grafana)

#### Core Raft Metrics

**Consensus State Metrics:**
```
# Current Raft term (increases with elections)
raft_current_term{node_id="node-1"}

# Index of highest log entry known to be committed
raft_commit_index{node_id="node-1"}

# Index of highest log entry applied to state machine
raft_last_applied{node_id="node-1"}

# Current role: 1=leader, 2=candidate, 3=follower
raft_role{node_id="node-1"}

# ID of current leader (0 if unknown)
raft_leader_id{node_id="node-1"}

# Current election timeout in milliseconds
raft_election_timeout_ms{node_id="node-1"}
```

**Performance Metrics:**
```
# Total number of log entries in the log
raft_log_entries_total{node_id="node-1"}

# Number of heartbeats sent (leader only)
raft_heartbeat_count{node_id="node-1"}

# Total number of log compactions performed
raft_log_compaction_count{node_id="node-1"}

# Total number of snapshots installed from other nodes
raft_snapshot_install_count{node_id="node-1"}

# RPC duration histogram by type
raft_rpc_duration_seconds{type="vote_request|append_entries|install_snapshot"}

# Total number of elections started
raft_election_count_total{node_id="node-1"}
```

**Message Metrics:**
```
# Messages sent by type
raft_messages_sent_total{node_id="node-1",type="vote_req|vote_resp|append_entries|install_snapshot"}

# Messages received by type
raft_messages_received_total{node_id="node-1",type="vote_req|vote_resp|append_entries|install_snapshot"}

# Message failures
raft_message_failures_total{node_id="node-1",type="timeout|network_error"}
```

**Replication Lag Metrics:**
```
# Lag in log entries from leader (follower perspective)
raft_replication_lag_entries{node_id="node-1",leader_id="node-2"}

# Time since last successful AppendEntries
raft_last_append_seconds{node_id="node-1"}
```

#### Implementation in Observer Trait

```rust
// In your observer trait or implementation
pub trait MetricsObserver {
    fn record_term(&self, term: u64);
    fn record_commit_index(&self, index: u64);
    fn record_role_change(&self, role: RaftRole);
    fn record_rpc_duration(&self, rpc_type: RpcType, duration: Duration);
    fn increment_message_count(&self, msg_type: MessageType, direction: Direction);
    fn record_replication_lag(&self, lag_entries: u64);
}
```

#### AWS Integration Options

**Option 1: Amazon Managed Prometheus (AMP)**
- **Pros**:
  - No server management
  - Prometheus-compatible (standard PromQL)
  - Automatic scaling
  - Integration with Managed Grafana
- **Cons**:
  - Higher cost (~$0.10 per million samples ingested)
  - Limited local querying
- **Best For**: Production deployments prioritizing operational simplicity

**Option 2: Self-Hosted Prometheus on ECS/EKS**
- **Pros**:
  - Lower cost
  - Full control over configuration
  - Local metrics storage
- **Cons**:
  - Requires operational management
  - Need to handle high availability
- **Best For**: Cost-sensitive deployments with Prometheus expertise

**Option 3: CloudWatch Container Insights**
- **Pros**:
  - Native AWS integration
  - Zero setup for basic metrics
  - Unified dashboard with other AWS services
- **Cons**:
  - Limited custom metrics
  - Higher query latency
  - Less flexible than Prometheus
- **Best For**: Quick start, AWS-centric monitoring

#### Recommended Grafana Dashboards

**Dashboard 1: Cluster Health**
- Current leader
- Node roles (visual indicator)
- Election history timeline
- Time since last leader election
- Cluster availability percentage

**Dashboard 2: Replication Status**
- Commit index across all nodes (line graph)
- Replication lag per follower
- Log growth rate
- Snapshot creation frequency

**Dashboard 3: Performance**
- RPC latency percentiles (p50, p95, p99)
- Messages per second
- State machine application rate
- Throughput (commands/sec)

**Dashboard 4: Resource Utilization**
- CPU usage per node
- Memory usage
- Disk I/O
- Network bandwidth

---

### Distributed Tracing (OpenTelemetry)

#### Key Traces to Implement

**1. Client Request Trace:**
```
Client Request → Leader Receives → Log Appended → Replicated to Followers →
Quorum Achieved → Committed → Applied to State Machine → Response to Client

Spans:
- receive_client_request (duration: 0-2ms)
- append_to_log (duration: 1-5ms)
- replicate_to_followers (duration: 10-50ms) [parent span]
  - send_append_entries_node_2 (duration: 5-15ms)
  - send_append_entries_node_3 (duration: 5-15ms)
- wait_for_commit (duration: 0-20ms)
- apply_to_state_machine (duration: 1-10ms)
- send_response (duration: 1-2ms)
```

**2. Election Trace:**
```
Election Timeout → Become Candidate → Send Vote Requests →
Receive Votes → Quorum Achieved → Become Leader → Send Heartbeats

Spans:
- election_triggered (attributes: timeout_ms, previous_term)
- request_votes (parallel child spans per node)
- count_votes (attributes: votes_received, votes_needed)
- transition_to_leader (attributes: new_term)
```

**3. Snapshot Installation Trace:**
```
Snapshot Triggered → Create Snapshot → Transfer to Follower →
Follower Applies Snapshot → Confirms Installation

Spans:
- trigger_snapshot (attributes: last_included_index)
- create_snapshot (duration: 100-500ms)
- transfer_snapshot (duration: varies by size)
  - send_chunk_1
  - send_chunk_2
  - ...
- apply_snapshot (follower side)
```

#### Implementation Approach

```rust
// New trait for tracing integration
pub trait TracingObserver {
    type SpanId;

    fn start_span(&self, name: &str, parent: Option<Self::SpanId>) -> Self::SpanId;
    fn add_span_attribute(&self, span: Self::SpanId, key: &str, value: &str);
    fn end_span(&self, span: Self::SpanId, status: SpanStatus);
}

// Usage in RaftNode
fn handle_client_request(&mut self, request: ClientRequest) {
    let span = self.observer.start_span("handle_client_request", None);
    self.observer.add_span_attribute(span, "node_id", &self.id.to_string());

    // ... process request ...

    self.observer.end_span(span, SpanStatus::Ok);
}
```

#### AWS Integration Options

**Option 1: AWS X-Ray**
- **Pros**:
  - Native AWS integration
  - Automatic AWS service tracing
  - Service map visualization
  - Low overhead
- **Cons**:
  - Less detailed than Jaeger for internal logic
  - Sampling limitations
- **Best For**: Understanding request flow through AWS services
- **Integration**: Use `opentelemetry-aws` crate with X-Ray exporter

**Option 2: Jaeger on ECS/EKS**
- **Pros**:
  - More detailed internal tracing
  - Better for debugging consensus issues
  - Open-source flexibility
  - Rich filtering and search
- **Cons**:
  - Additional infrastructure to manage
  - Higher operational complexity
- **Best For**: Deep debugging of Raft internals
- **Deployment**: Run Jaeger as sidecar or standalone service

**Option 3: Hybrid Approach (Recommended)**
- Use X-Ray for high-level request tracing
- Use Jaeger for detailed Raft-specific debugging
- Configure sampling (100% in dev, 1-10% in production)

---

### Structured Logging (tracing crate)

#### Implementation with Tokio Tracing

```rust
use tracing::{info, warn, error, debug, instrument};

// Instrument functions to automatically create spans
#[instrument(skip(self), fields(
    node_id = %self.id,
    term = self.current_term,
    role = ?self.role
))]
fn handle_vote_request(&mut self, req: VoteRequest) -> VoteResponse {
    info!(
        candidate = %req.candidate_id,
        candidate_term = req.term,
        "Processing vote request"
    );

    if req.term < self.current_term {
        warn!(
            candidate = %req.candidate_id,
            req_term = req.term,
            current_term = self.current_term,
            "Rejecting vote: stale term"
        );
        return VoteResponse::rejected(self.current_term);
    }

    // ... voting logic ...

    info!(
        candidate = %req.candidate_id,
        vote_granted = true,
        "Vote granted"
    );

    VoteResponse::granted(self.current_term)
}

#[instrument(skip(self))]
fn become_leader(&mut self) {
    info!(
        term = self.current_term,
        "Transitioning to leader role"
    );

    self.role = RaftRole::Leader;
    self.initialize_leader_state();

    info!(
        next_index = ?self.leader_state.next_index,
        "Leader state initialized"
    );
}
```

#### Log Format (Structured JSON)

```json
{
  "timestamp": "2026-02-02T14:23:45.123Z",
  "level": "INFO",
  "target": "metal_raft::raft_node",
  "fields": {
    "message": "Processing vote request",
    "node_id": "node-1",
    "term": 5,
    "role": "Follower",
    "candidate": "node-2",
    "candidate_term": 6
  },
  "span": {
    "name": "handle_vote_request",
    "node_id": "node-1",
    "term": 5
  }
}
```

#### AWS Integration

**CloudWatch Logs:**
- **Setup**: Use `tracing-subscriber` with JSON formatter
- **Log Groups**: One per service (e.g., `/aws/ecs/metal-raft`)
- **Retention**: 30 days for development, 90+ days for production
- **Cost**: $0.50/GB ingested, $0.03/GB stored

**CloudWatch Logs Insights Queries:**

```sql
-- Find all elections in the last hour
fields @timestamp, node_id, term, candidate
| filter @message like /Transitioning to leader/
| sort @timestamp desc

-- Find vote rejections by reason
fields @timestamp, node_id, candidate, reason
| filter @message like /Rejecting vote/
| stats count() by reason

-- Identify slow log replications
fields @timestamp, node_id, duration
| filter @message like /AppendEntries completed/
| filter duration > 100
| sort duration desc
```

**Log Levels:**
- **ERROR**: Unrecoverable errors (corruption, panic)
- **WARN**: Recoverable issues (stale RPCs, vote rejections)
- **INFO**: State changes (elections, role transitions, snapshots)
- **DEBUG**: Detailed operations (individual RPCs, quorum checks)
- **TRACE**: Verbose debugging (every message, state inspection)

**Production Configuration:**
- Default: INFO level
- Enable DEBUG for specific nodes during troubleshooting
- TRACE only in development/testing

---

### Alerting

#### Critical Alerts

**1. No Leader Available**
- **Condition**: No node has `raft_role = 1` for >10 seconds
- **Severity**: Critical (P1)
- **Impact**: Cluster cannot accept writes
- **Response**: Page on-call engineer
- **Auto-Remediation**: Restart nodes in sequence

**2. High Replication Lag**
- **Condition**: `raft_replication_lag_entries > 1000` for >5 minutes
- **Severity**: High (P2)
- **Impact**: Follower may require snapshot (expensive)
- **Response**: Investigate network or slow follower

**3. Snapshot Installation Failures**
- **Condition**: `raft_snapshot_install_count` not increasing, but lag growing
- **Severity**: High (P2)
- **Impact**: Follower cannot catch up
- **Response**: Check network, disk space, logs

**4. Frequent Elections**
- **Condition**: `rate(raft_election_count_total[5m]) > 2`
- **Severity**: Medium (P3)
- **Impact**: Reduced availability, potential network partition
- **Response**: Check network latency, node health

**5. Commit Index Stalled**
- **Condition**: `raft_commit_index` unchanged for >60 seconds (with active writes)
- **Severity**: Critical (P1)
- **Impact**: No progress on client requests
- **Response**: Check leader health, quorum availability

**6. State Machine Application Lag**
- **Condition**: `raft_commit_index - raft_last_applied > 1000`
- **Severity**: Medium (P3)
- **Impact**: Stale reads, slow response times
- **Response**: Check state machine performance

#### Warning Alerts

**1. Elevated Election Timeout**
- **Condition**: `raft_election_timeout_ms > 1000` consistently
- **Severity**: Low (P4)
- **Impact**: Slower leader election during failures
- **Response**: Review election timeout configuration

**2. High RPC Latency**
- **Condition**: `raft_rpc_duration_seconds{quantile="0.95"} > 0.1`
- **Severity**: Medium (P3)
- **Impact**: Degraded performance
- **Response**: Check network, node CPU/memory

**3. Disk Space Low**
- **Condition**: EBS volume >80% full
- **Severity**: Medium (P3)
- **Impact**: Snapshots may fail, log growth limited
- **Response**: Trigger manual compaction or increase volume size

#### AWS Integration

**CloudWatch Alarms:**
```yaml
# Example alarm configuration
NoLeaderAlarm:
  MetricName: raft_role
  Statistic: Maximum
  Period: 30 seconds
  EvaluationPeriods: 2
  Threshold: 0  # No node has role = 1 (leader)
  ComparisonOperator: LessThanThreshold
  TreatMissingData: breaching
  Actions:
    - SNS Topic: arn:aws:sns:us-east-1:123456789:critical-alerts
```

**SNS (Simple Notification Service):**
- **Critical alerts** → PagerDuty integration
- **High severity** → Slack channel
- **Medium/Low** → Email to team distribution list

**EventBridge Rules:**
- Trigger Lambda for auto-remediation
- Example: Restart task if health check fails 3 times

**AWS Personal Health Dashboard:**
- Alerts for AWS service issues affecting deployment
- Proactive notifications for maintenance events

---

## Implementation Plan

### Phase 1: AWS Runtime Realization

Create a new `aws/` folder as the third realization of the MetalRaft core (alongside `embassy/` and `validation/`).

#### Folder Structure

```
metal_raft/aws/
  Cargo.toml
  README.md
  Dockerfile
  src/
    main.rs                    # Binary entry point
    aws_node.rs                # High-level node wrapper
    tokio_timer.rs             # TimerService trait implementation
    grpc_transport.rs          # Transport trait with tonic
    ebs_storage.rs             # Storage trait with file system persistence
    s3_snapshot_archiver.rs    # S3 integration for snapshot backup
    prometheus_observer.rs     # Observer trait with Prometheus metrics
    tracing_observer.rs        # OpenTelemetry tracing integration
    logging.rs                 # Structured logging setup
    config.rs                  # Configuration loading (env vars, files)
    health_check.rs            # HTTP health endpoint for load balancers
  proto/
    raft.proto                 # gRPC service definitions
  terraform/                   # Infrastructure as Code
    main.tf
    vpc.tf
    ecs.tf                     # Or eks.tf
    load_balancer.tf
    monitoring.tf
    variables.tf
    outputs.tf
  k8s/                         # Kubernetes manifests (if using EKS)
    statefulset.yaml
    service.yaml
    configmap.yaml
  ecs/                         # ECS task definitions (if using Fargate)
    task-definition.json
    service.json
  scripts/
    build.sh                   # Build Docker image
    deploy.sh                  # Deploy to AWS
    test-local.sh              # Run locally with Docker Compose
  docker-compose.yml           # Local testing setup
```

#### Key Implementation Files

**Cargo.toml:**
```toml
[package]
name = "metal-raft-aws"
version = "0.1.0"
edition = "2021"

[dependencies]
metal-raft-core = { path = "../core" }
tokio = { version = "1.35", features = ["full"] }
tonic = "0.10"
prost = "0.12"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["json", "env-filter"] }
tracing-opentelemetry = "0.22"
opentelemetry = "0.21"
opentelemetry-aws = "0.9"
prometheus = "0.13"
axum = "0.7"  # For HTTP health check endpoint
aws-sdk-s3 = "1.10"
anyhow = "1.0"
thiserror = "1.0"

[build-dependencies]
tonic-build = "0.10"
```

**src/tokio_timer.rs:**
```rust
use metal_raft_core::timer_service::{TimerService, TimerId};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

pub struct TokioTimer {
    timers: Arc<Mutex<HashMap<TimerId, tokio::task::JoinHandle<()>>>>,
}

impl TokioTimer {
    pub fn new() -> Self {
        Self {
            timers: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl TimerService for TokioTimer {
    fn set_timer(&mut self, id: TimerId, duration: Duration, callback: Box<dyn FnOnce()>) {
        let handle = tokio::spawn(async move {
            sleep(duration).await;
            callback();
        });

        self.timers.lock().await.insert(id, handle);
    }

    fn cancel_timer(&mut self, id: TimerId) {
        if let Some(handle) = self.timers.lock().await.remove(&id) {
            handle.abort();
        }
    }
}
```

**src/grpc_transport.rs:**
```rust
use metal_raft_core::transport::{Transport, Message};
use tonic::{Request, Response, Status};
use std::collections::HashMap;

pub struct GrpcTransport {
    node_id: NodeId,
    peers: HashMap<NodeId, RaftServiceClient<Channel>>,
}

#[tonic::async_trait]
impl Transport for GrpcTransport {
    async fn send(&mut self, to: NodeId, message: Message) -> Result<(), TransportError> {
        let client = self.peers.get_mut(&to).ok_or(TransportError::UnknownNode)?;

        match message {
            Message::VoteRequest(req) => {
                client.request_vote(Request::new(req.into())).await?;
            }
            Message::AppendEntries(req) => {
                client.append_entries(Request::new(req.into())).await?;
            }
            // ... other message types
        }

        Ok(())
    }
}
```

**src/prometheus_observer.rs:**
```rust
use metal_raft_core::observer::Observer;
use prometheus::{Registry, Gauge, Counter, Histogram, HistogramOpts};

pub struct PrometheusObserver {
    registry: Registry,
    current_term: Gauge,
    commit_index: Gauge,
    role: Gauge,
    election_count: Counter,
    rpc_duration: Histogram,
    // ... other metrics
}

impl PrometheusObserver {
    pub fn new(node_id: NodeId) -> Self {
        let registry = Registry::new();

        let current_term = Gauge::new("raft_current_term", "Current Raft term")
            .unwrap();
        registry.register(Box::new(current_term.clone())).unwrap();

        // ... register other metrics

        Self {
            registry,
            current_term,
            // ...
        }
    }

    pub fn metrics(&self) -> String {
        let encoder = prometheus::TextEncoder::new();
        let families = self.registry.gather();
        encoder.encode_to_string(&families).unwrap()
    }
}

impl Observer for PrometheusObserver {
    fn on_term_change(&self, new_term: u64) {
        self.current_term.set(new_term as f64);
    }

    fn on_commit_index_change(&self, new_index: u64) {
        self.commit_index.set(new_index as f64);
    }

    // ... other observer methods
}
```

**Dockerfile:**
```dockerfile
FROM rust:1.75 as builder

WORKDIR /app
COPY . .

# Build with musl for static binary
RUN rustup target add x86_64-unknown-linux-musl
RUN cargo build --release --target x86_64-unknown-linux-musl --package metal-raft-aws

FROM alpine:3.19
RUN apk --no-cache add ca-certificates

COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/metal-raft-aws /usr/local/bin/raft-node

EXPOSE 8080 9090
ENTRYPOINT ["raft-node"]
```

---

### Phase 2: Observability Integration

#### Step 1: Implement Prometheus Metrics

1. Extend `PrometheusObserver` with all metrics defined in the Observability Strategy section
2. Create `/metrics` HTTP endpoint using Axum
3. Configure Prometheus scraping (15-30 second intervals)

**Health Check Endpoint (src/health_check.rs):**
```rust
use axum::{routing::get, Router};
use prometheus::Encoder;

pub fn create_health_router(observer: Arc<PrometheusObserver>) -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/metrics", get(move || metrics_handler(observer.clone())))
}

async fn health_check() -> &'static str {
    "OK"
}

async fn metrics_handler(observer: Arc<PrometheusObserver>) -> String {
    observer.metrics()
}
```

#### Step 2: Add OpenTelemetry Tracing

1. Implement `TracingObserver` trait in `src/tracing_observer.rs`
2. Configure X-Ray exporter for AWS integration
3. Add span context propagation in gRPC messages
4. Set sampling rate (100% dev, 5-10% prod)

**Tracing Setup (src/logging.rs):**
```rust
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use opentelemetry::trace::TracerProvider;
use opentelemetry_aws::trace::XRayPropagator;

pub fn init_tracing(service_name: &str) -> anyhow::Result<()> {
    // Set up X-Ray propagator
    opentelemetry::global::set_text_map_propagator(XRayPropagator::default());

    // Create X-Ray tracer
    let tracer = opentelemetry_aws::trace::XRayTracerProviderBuilder::default()
        .with_service_name(service_name)
        .build()?;

    // Set up tracing subscriber with JSON formatting
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .with(tracing_subscriber::fmt::layer().json())
        .with(tracing_opentelemetry::layer().with_tracer(tracer))
        .init();

    Ok(())
}
```

#### Step 3: Configure Structured Logging

1. Set up `tracing-subscriber` with JSON formatter
2. Configure CloudWatch Logs integration
3. Define log levels per environment
4. Create CloudWatch Logs Insights query templates

---

### Phase 3: AWS Infrastructure (Terraform)

#### Terraform Configuration

**terraform/main.tf:**
```hcl
terraform {
  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
  }

  backend "s3" {
    bucket = "metal-raft-terraform-state"
    key    = "production/terraform.tfstate"
    region = "us-east-1"
  }
}

provider "aws" {
  region = var.aws_region
}

module "vpc" {
  source = "./modules/vpc"

  cidr_block = var.vpc_cidr
  azs        = var.availability_zones
}

module "ecs_cluster" {
  source = "./modules/ecs"

  cluster_name    = "metal-raft-cluster"
  vpc_id          = module.vpc.vpc_id
  private_subnets = module.vpc.private_subnets
  node_count      = var.raft_node_count
  container_image = var.container_image
}

module "load_balancer" {
  source = "./modules/load_balancer"

  vpc_id          = module.vpc.vpc_id
  public_subnets  = module.vpc.public_subnets
  target_group_id = module.ecs_cluster.target_group_id
}

module "monitoring" {
  source = "./modules/monitoring"

  cluster_name = module.ecs_cluster.cluster_name
  log_group    = "/aws/ecs/metal-raft"
}
```

**terraform/ecs.tf (ECS Fargate Module):**
```hcl
resource "aws_ecs_cluster" "main" {
  name = var.cluster_name
}

resource "aws_ecs_task_definition" "raft_node" {
  family                   = "metal-raft-node"
  network_mode             = "awsvpc"
  requires_compatibilities = ["FARGATE"]
  cpu                      = "256"
  memory                   = "512"
  execution_role_arn       = aws_iam_role.ecs_execution.arn
  task_role_arn            = aws_iam_role.ecs_task.arn

  container_definitions = jsonencode([{
    name  = "raft-node"
    image = var.container_image

    portMappings = [
      { containerPort = 8080, protocol = "tcp" },  # gRPC
      { containerPort = 9090, protocol = "tcp" }   # Metrics
    ]

    environment = [
      { name = "NODE_ID", value = "node-1" },
      { name = "CLUSTER_SIZE", value = "3" },
      { name = "LOG_LEVEL", value = "info" }
    ]

    mountPoints = [{
      sourceVolume  = "raft-data"
      containerPath = "/data"
    }]

    logConfiguration = {
      logDriver = "awslogs"
      options = {
        "awslogs-group"         = "/aws/ecs/metal-raft"
        "awslogs-region"        = var.aws_region
        "awslogs-stream-prefix" = "ecs"
      }
    }
  }])

  volume {
    name = "raft-data"

    efs_volume_configuration {
      file_system_id = aws_efs_file_system.raft_data.id
      root_directory = "/node-1"
    }
  }
}

resource "aws_ecs_service" "raft_nodes" {
  name            = "metal-raft-service"
  cluster         = aws_ecs_cluster.main.id
  task_definition = aws_ecs_task_definition.raft_node.arn
  desired_count   = var.node_count
  launch_type     = "FARGATE"

  network_configuration {
    subnets         = var.private_subnets
    security_groups = [aws_security_group.raft_nodes.id]
  }

  service_registries {
    registry_arn = aws_service_discovery_service.raft.arn
  }

  load_balancer {
    target_group_arn = aws_lb_target_group.raft_clients.arn
    container_name   = "raft-node"
    container_port   = 8080
  }
}
```

**Service Discovery:**
```hcl
resource "aws_service_discovery_private_dns_namespace" "main" {
  name = "raft.local"
  vpc  = var.vpc_id
}

resource "aws_service_discovery_service" "raft" {
  name = "nodes"

  dns_config {
    namespace_id = aws_service_discovery_private_dns_namespace.main.id

    dns_records {
      ttl  = 10
      type = "A"
    }
  }

  health_check_custom_config {
    failure_threshold = 1
  }
}
```

---

## Quick Wins

### 1. Start with ECS Fargate
- **Rationale**: Fastest path from development to production
- **Timeline**: 1-2 weeks for basic deployment
- **Benefits**: Minimal infrastructure management, automatic scaling, built-in monitoring
- **Next Steps**:
  - Create Dockerfile for Rust binary
  - Set up ECR (Elastic Container Registry)
  - Deploy 3-node cluster with CloudFormation/Terraform

### 2. Use `prometheus` Crate
- **Rationale**: Standard Rust library with minimal overhead
- **Implementation**: ~50-100 lines to extend Observer trait
- **Benefits**: Industry-standard format, works with Grafana
- **Next Steps**:
  - Add `prometheus = "0.13"` to Cargo.toml
  - Implement metrics in existing Observer trait
  - Expose `/metrics` endpoint

### 3. CloudWatch Logs First
- **Rationale**: Zero setup, native AWS integration
- **Timeline**: Immediate (built into ECS)
- **Benefits**: Centralized logs, query with Logs Insights, easy alerting
- **Evolution Path**: Graduate to X-Ray for tracing, then Jaeger for deep debugging
- **Next Steps**:
  - Configure `tracing-subscriber` with JSON format
  - Set up CloudWatch Log Group
  - Create basic Logs Insights queries

### 4. Test with LocalStack
- **Rationale**: Simulate AWS locally before deploying
- **Tools**: LocalStack, Docker Compose
- **Benefits**: Faster iteration, no AWS costs during development
- **Next Steps**:
  - Install LocalStack: `pip install localstack`
  - Create `docker-compose.yml` with S3, CloudWatch, X-Ray mocks
  - Run integration tests against local AWS services

---

## Recommendations

### 1. Create `aws/` Folder as Third Realization
Follow the existing pattern established by `embassy/` and `validation/`:
- Keep core frozen (no changes to `core/`)
- Implement all AWS-specific concerns in `aws/` realization
- Use dependency injection for all I/O (Storage, Transport, TimerService)
- Maintain same test coverage as other realizations

### 2. Use Tokio + tonic (gRPC) for the Runtime
- **Tokio**: De facto async runtime in Rust, excellent ecosystem
- **tonic**: High-performance gRPC implementation
- **Benefits**:
  - Efficient binary protocol (gRPC)
  - Built-in load balancing
  - Strong typing with Protocol Buffers
  - HTTP/2 multiplexing
- **Alternative**: If you prefer REST, use Axum + JSON

### 3. Deploy on ECS Fargate Initially
- **Rationale**: Simpler than K8s for consensus layer (3-5 nodes)
- **When to Switch to EKS**:
  - Need StatefulSets with strict ordering
  - Multi-region federation required
  - Team prefers K8s ecosystem
- **Migration Path**: Core stays the same, only deployment manifests change

### 4. Prometheus Metrics in Observer Trait
- **Perfect Fit**: Your existing Observer trait is ideal for metrics
- **Implementation**:
  - `PrometheusObserver` implements `Observer`
  - Each observer method increments/sets a metric
  - Expose `/metrics` endpoint for scraping
- **No Core Changes**: Zero modifications to frozen core

### 5. Keep the Core Frozen
- **Principle**: All AWS concerns stay in `aws/` folder
- **Benefits**:
  - Core remains portable across platforms
  - Easier to reason about correctness
  - Validation tests still pass unchanged
- **If Core Changes Needed**: Abstract properly with new traits (Storage, Transport extensions)

---

## Next Steps & Action Items

### Immediate (Week 1-2)
- [ ] Create `aws/` folder structure
- [ ] Implement `TokioTimer` (TimerService trait)
- [ ] Implement basic `GrpcTransport` with tonic
- [ ] Create Dockerfile with multi-stage build
- [ ] Run 3-node cluster locally with Docker Compose

### Short-Term (Week 3-4)
- [ ] Implement `PrometheusObserver` with core metrics
- [ ] Add structured logging with `tracing`
- [ ] Create basic Terraform configuration for VPC + ECS
- [ ] Deploy to AWS Fargate (dev environment)
- [ ] Set up CloudWatch Logs and basic alerting

### Medium-Term (Month 2-3)
- [ ] Implement EBS-backed storage with S3 snapshot archival
- [ ] Add OpenTelemetry tracing with X-Ray
- [ ] Create Grafana dashboards for cluster health
- [ ] Implement health check endpoints for load balancers
- [ ] Production deployment with multi-AZ high availability

### Long-Term (Month 4+)
- [ ] Performance testing and optimization
- [ ] Multi-region replication (if needed)
- [ ] Advanced monitoring (anomaly detection, predictive alerts)
- [ ] Cost optimization (spot instances, autoscaling)
- [ ] Documentation and runbooks

---

## Cost Estimation

### Monthly AWS Costs (3-Node Cluster)

**ECS Fargate Option:**
- Fargate tasks: 3 nodes × 0.25 vCPU × 0.5GB × $0.04/vCPU-hour × 730 hours = ~$45
- Network Load Balancer: $16.20
- EBS volumes: 3 × 20GB × $0.08/GB = $4.80
- S3 snapshots: 10GB × $0.023/GB = $0.23
- CloudWatch Logs: 5GB × $0.50/GB = $2.50
- Data transfer: ~$5
- **Total: ~$75/month**

**EKS Option:**
- EKS control plane: $73
- EC2 worker nodes: 2 × t3.medium × $0.0416/hour × 730 hours = $61
- EBS volumes: $4.80
- S3 snapshots: $0.23
- Load Balancer: $16.20
- **Total: ~$155/month**

**Cost Optimization Tips:**
- Use Savings Plans or Reserved Instances (40-60% discount)
- Enable EBS snapshots lifecycle policies
- Use S3 Intelligent-Tiering for snapshots
- Configure autoscaling to reduce capacity during low traffic

---

## References & Resources

### AWS Documentation
- [ECS Best Practices](https://docs.aws.amazon.com/AmazonECS/latest/bestpracticesguide/intro.html)
- [Fargate Task Definitions](https://docs.aws.amazon.com/AmazonECS/latest/developerguide/task_definitions.html)
- [CloudWatch Container Insights](https://docs.aws.amazon.com/AmazonCloudWatch/latest/monitoring/ContainerInsights.html)

### Rust Crates
- [tokio](https://tokio.rs/) - Async runtime
- [tonic](https://github.com/hyperium/tonic) - gRPC framework
- [prometheus](https://docs.rs/prometheus/) - Metrics library
- [tracing](https://docs.rs/tracing/) - Structured logging
- [aws-sdk-rust](https://github.com/awslabs/aws-sdk-rust) - AWS SDK

### Observability
- [Prometheus Best Practices](https://prometheus.io/docs/practices/naming/)
- [OpenTelemetry Rust](https://github.com/open-telemetry/opentelemetry-rust)
- [Grafana Dashboard Examples](https://grafana.com/grafana/dashboards/)

### Infrastructure as Code
- [Terraform AWS Provider](https://registry.terraform.io/providers/hashicorp/aws/latest/docs)
- [AWS CDK Examples](https://github.com/aws-samples/aws-cdk-examples)

---

## Questions & Discussion

### Open Questions
1. **Client Protocol**: REST or gRPC? (Recommendation: gRPC for efficiency)
2. **State Machine**: What application will run on top of Raft? (KV store, distributed lock, etc.)
3. **Scale**: Expected cluster size? (3-5 nodes initially recommended)
4. **Regions**: Single region or multi-region? (Start with single region)
5. **Compliance**: Any specific requirements (encryption, audit logs)?

### Design Decisions to Make
1. **Snapshot Storage**: EBS only, or EBS + S3 archival?
2. **Service Discovery**: AWS Cloud Map or Consul?
3. **Monitoring**: Managed Prometheus or self-hosted?
4. **Tracing**: X-Ray, Jaeger, or both?
5. **Deployment**: Blue/green, rolling, or canary?

### Performance Targets
- Target consensus latency: <50ms (p95)
- Target throughput: 1000+ ops/second
- Target availability: 99.9% (3-node cluster with multi-AZ)
- Recovery time objective (RTO): <5 minutes
- Recovery point objective (RPO): <1 minute

---

## Conclusion

MetalRaft is well-positioned for AWS deployment thanks to its technology-agnostic core design. The recommended approach is:

1. **Start Simple**: ECS Fargate + CloudWatch Logs + Prometheus metrics
2. **Iterate Fast**: Test locally with Docker Compose before AWS deployment
3. **Add Sophistication Gradually**: X-Ray → Jaeger, Managed Prometheus → custom dashboards
4. **Keep Core Frozen**: All AWS concerns in `aws/` realization layer
5. **Leverage Existing Architecture**: Observer trait is perfect for metrics integration

The path forward is clear: implement the AWS realization following the same pattern as the Embassy embedded realization, but with Tokio/gRPC instead of Embassy/UDP. The core Raft algorithm remains unchanged and portable.

---

**Document Status**: Initial planning document (February 2, 2026)
**Next Review**: After Phase 1 implementation
**Owner**: TBD
**Contributors**: TBD
