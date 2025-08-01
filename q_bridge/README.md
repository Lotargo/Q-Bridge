# Quantum Bridge (Q-Bridge) - Phase 1: The Core

This repository contains the foundational implementation for the Quantum Bridge (Q-Bridge) project. This first phase, "The Core," establishes a high-performance, reliable transport layer for data requests.

## Project Structure

The project is a Cargo workspace containing the following crates:

-   `gateway`: A service that exposes gRPC and REST endpoints for receiving client requests. It pushes requests into a Redis stream for processing.
-   `buffer`: A consumer service that reads from the Redis stream, deserializes requests, and (currently) logs them. This service is designed to be the entry point for the future ML orchestration layer.
-   `transport`: A placeholder proof-of-concept for a high-performance, zero-copy data transport layer using Apache Arrow Flight.
-   `common`: A shared library crate containing the protobuf definitions used by the `gateway` and `buffer`.

## How to Build and Run

### Prerequisites

-   Rust (latest stable version)
-   Docker and Docker Compose (or `docker compose`)
-   Python 3.x with `pip`

### 1. Build the Rust Services

From the `q_bridge` directory, build all the services:

```bash
cargo build --release
```

### 2. Start Dependencies

The system requires a Redis instance. A `docker-compose.yml` file is provided to run Redis easily.

```bash
docker compose up -d
```

### 3. Run the Services

Each service can be run in a separate terminal from the `q_bridge` directory:

**Gateway:**
```bash
cargo run --release --bin gateway
```

**Buffer:**
```bash
cargo run --release --bin buffer
```

**Transport:**
```bash
cargo run --release --bin transport
```

### 4. Run the End-to-End Test

An E2E test script is provided to verify the system's functionality.

First, install the Python dependencies:
```bash
pip install -r scripts/requirements.txt
```

Then, run the test script. The script will automatically start and stop all the necessary services.
**Note:** The user running the script must have permissions to access the Docker daemon.

```bash
python3 scripts/e2e_test_client.py
```

---

## Handoff Notes for Phase 2

This section details the interfaces and components for the team working on Phase 2: The Intelligence.

### Key Components & Ports

| Service   | Crate       | Port    | Protocol | Description                                                              |
| :-------- | :---------- | :------ | :------- | :----------------------------------------------------------------------- |
| **Gateway** | `gateway`   | `3000`  | REST     | Primary entry point for REST clients. Accepts JSON payloads.             |
| **Gateway** | `gateway`   | `50051` | gRPC     | Primary entry point for gRPC clients.                                    |
| **Transport** | `transport` | `50052` | gRPC     | Mock Apache Arrow Flight service.                                        |
| **Redis**   | `(docker)`  | `6379`  | TCP      | Message broker used for buffering requests.                              |

### Data Flow

1.  A client sends a request to the **Gateway** service via either the REST or gRPC endpoint.
2.  The **Gateway** serializes the request into a protobuf message (`InternalRequest`) and pushes it as a new entry into a Redis Stream named `q_bridge_stream`.
3.  The **Buffer** service is part of a consumer group (`q_bridge_group`) reading from this stream. It reads the `InternalRequest` message, deserializes it, and logs its content.

### Phase 2 Integration Point

The **`buffer`** crate is the primary integration point for Phase 2.

-   **File:** `q_bridge/buffer/src/main.rs`
-   **Function:** `process_message()`

Currently, `process_message()` only logs the deserialized request. This function should be modified to:
1.  Call the new RL-based Orchestrator service.
2.  Pass the `InternalRequest` to the orchestrator.
3.  Receive instructions from the orchestrator (e.g., how to batch, when to send to a database).
4.  Execute those instructions, which will likely involve interacting with the `transport` layer.

### `InternalRequest` Protobuf Schema

The shared data structure is defined in `q_bridge/common/proto/gateway.proto`.

```proto
syntax = "proto3";

package gateway;

message InternalRequest {
  string request_id = 1;
  string agent_id = 2;
  bytes payload = 3;
  map<string, string> metadata = 4;
}
```

This structure is used for all internal communication via the Redis buffer. The `payload` is application-specific and will contain the core query or data from the client. The `metadata` can be used by the orchestrator for routing or prioritization decisions.
