# P1-GATEWAY-SIMPLE

This project is a simplified, high-performance API Gateway written in Go. It listens for HTTP POST requests, enriches the JSON payload with metadata, serializes it to Msgpack, and publishes it to a Redis queue.

## 1. Prerequisites

- Go (version 1.18 or later)
- Docker and Docker Compose
- `curl` for testing

## 2. Getting Started

### 2.1. Clone the Repository

```bash
git clone <repository-url>
cd p1-gateway-simple
```

### 2.2. Install Dependencies

The project uses Go modules. The dependencies will be downloaded automatically when you build or run the application.

## 3. Running the Service

### 3.1. Start Redis

The service requires a running Redis instance. A `docker-compose.yml` file is provided for convenience.

```bash
docker-compose up -d
```

This will start a Redis container and expose it on port `6379`.

### 3.2. Run the Go Application

You can run the application directly using `go run`:

```bash
go run main.go
```

The server will start and listen on port `8080`.

```
2024/07/31 12:00:00 Server starting on :8080
```

## 4. Testing

### 4.1. Running Unit Tests

The project includes unit tests that verify the handler's logic. These tests use an in-memory Redis server and do not require the Docker container to be running.

```bash
go test -v ./...
```

### 4.2. End-to-End Verification

You can test the running service using `curl`.

#### Test Script (curl)

Open a new terminal and send a POST request to the `/v1/submit` endpoint:

```bash
curl -X POST http://localhost:8080/v1/submit \
-H "Content-Type: application/json" \
-d '{
  "agent_id": "agent-007",
  "data": {
    "query": "What is the meaning of life?",
    "top_k": 5
  }
}'
```

#### Expected Success Response

If successful, the server will respond with an `HTTP 202 Accepted` status and a JSON body containing the `request_id`:

```json
{"request_id":"a1b2c3d4-e5f6-g7h8-i9j0-k1l2m3n4o5p6"}
```
*(Note: The `request_id` will be a unique UUID each time.)*

#### Verify in Redis

To verify that the message was published to the queue, you can use `redis-cli` to inspect the queue.

First, connect to the Redis container:
```bash
docker exec -it <container_name_or_id> redis-cli
```
*(You can find the container name with `docker ps`)*

Then, check the length of the queue:
```
127.0.0.1:6379> LLEN qbridge_simple_request_queue
(integer) 1
```

You can pop the message from the queue to inspect it. Since it's in Msgpack format, it will not be human-readable in the CLI.
```
127.0.0.1:6379> LPOP qbridge_simple_request_queue
"\x85\xaarequest_id\xd9$uuid...\xa9trace_id\xd9$uuid...\xa8agent_id\xa9agent-007..."
```

This confirms that the data was successfully processed and published.
