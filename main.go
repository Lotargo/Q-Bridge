package main

import (
	"context"
	"encoding/json"
	"gateway/internal/data"
	"log"
	"net/http"
	"time"

	"github.com/go-redis/redis/v8"
	"github.com/google/uuid"
	"github.com/vmihailenco/msgpack/v5"
)

const (
	redisAddr     = "redis://127.0.0.1/"
	redisQueue    = "qbridge_simple_request_queue"
	serverAddress = ":8080"
)

var ctx = context.Background()

func main() {
	// For this simple service, we'll use a single Redis client.
	// In a real-world app, you'd manage this more carefully (e.g., connection pooling).
	opt, err := redis.ParseURL(redisAddr)
	if err != nil {
		log.Fatalf("failed to parse redis URL: %v", err)
	}
	rdb := redis.NewClient(opt)

	http.HandleFunc("/v1/submit", submitHandler(rdb))

	log.Printf("Server starting on %s", serverAddress)
	if err := http.ListenAndServe(serverAddress, nil); err != nil {
		log.Fatalf("failed to start server: %v", err)
	}
}

func submitHandler(rdb *redis.Client) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		if r.Method != http.MethodPost {
			http.Error(w, "Only POST method is allowed", http.StatusMethodNotAllowed)
			return
		}

		// 1. Parse incoming JSON
		var payload data.IncomingPayload
		if err := json.NewDecoder(r.Body).Decode(&payload); err != nil {
			log.Printf("ERROR: Failed to parse JSON body: %v", err)
			http.Error(w, "Invalid JSON payload", http.StatusBadRequest)
			return
		}
		log.Printf("INFO: Received request for agent_id: %s", payload.AgentID)

		// 2. Generate metadata
		requestID := uuid.New().String()
		traceID := uuid.New().String() // In a real system, this might be propagated.
		timestamp := time.Now().UnixMilli()

		// 3. Create internal data structure
		internalData := data.InternalData{
			RequestID:   requestID,
			TraceID:     traceID,
			AgentID:     payload.AgentID,
			Payload:     payload.Data,
			TimestampMs: timestamp,
		}

		// 4. Serialize to Msgpack
		msgpackBytes, err := msgpack.Marshal(&internalData)
		if err != nil {
			log.Printf("ERROR: Failed to serialize data to Msgpack: %v", err)
			http.Error(w, "Internal server error", http.StatusInternalServerError)
			return
		}

		// 5. Publish to Redis
		err = rdb.RPush(ctx, redisQueue, msgpackBytes).Err()
		if err != nil {
			log.Printf("ERROR: Failed to publish message to Redis: %v", err)
			http.Error(w, "Internal server error", http.StatusInternalServerError)
			return
		}
		log.Printf("INFO: Published message for request_id: %s to queue: %s", requestID, redisQueue)

		// 6. Return success response
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusAccepted)
		json.NewEncoder(w).Encode(map[string]string{"request_id": requestID})
	}
}
