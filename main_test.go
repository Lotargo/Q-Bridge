package main

import (
	"encoding/json"
	"gateway/internal/data"
	"net/http"
	"net/http/httptest"
	"strings"
	"testing"

	"github.com/alicebob/miniredis/v2"
	"github.com/go-redis/redis/v8"
	"github.com/vmihailenco/msgpack/v5"
)

func TestSubmitHandler_Success(t *testing.T) {
	// Start a miniredis server
	mr, err := miniredis.Run()
	if err != nil {
		t.Fatalf("an error '%s' was not expected when starting miniredis", err)
	}
	defer mr.Close()

	// Create a Redis client pointing to the miniredis server
	rdb := redis.NewClient(&redis.Options{
		Addr: mr.Addr(),
	})

	// Create a test server
	handler := submitHandler(rdb)
	server := httptest.NewServer(handler)
	defer server.Close()

	// Create a request
	agentID := "test-agent-123"
	body := `{"agent_id": "` + agentID + `", "data": {"key": "value", "top_k": 10}}`
	req, err := http.NewRequest(http.MethodPost, server.URL, strings.NewReader(body))
	if err != nil {
		t.Fatalf("failed to create request: %v", err)
	}

	// Send the request
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		t.Fatalf("failed to send request: %v", err)
	}
	defer resp.Body.Close()

	// Check the status code
	if resp.StatusCode != http.StatusAccepted {
		t.Errorf("expected status %d, got %d", http.StatusAccepted, resp.StatusCode)
	}

	// Check the response body for request_id
	var respBody map[string]string
	if err := json.NewDecoder(resp.Body).Decode(&respBody); err != nil {
		t.Fatalf("failed to decode response body: %v", err)
	}
	if _, ok := respBody["request_id"]; !ok {
		t.Errorf("response body does not contain request_id")
	}

	// Check if the message was published to Redis
	values, err := mr.DB(0).List(redisQueue)
	if err != nil {
		t.Fatalf("failed to get list from miniredis: %v", err)
	}
	if len(values) != 1 {
		t.Fatalf("expected list length 1, got %d", len(values))
	}

	// Get the message from the queue and verify its contents
	pushedCmd, err := mr.Lpop(redisQueue)
	if err != nil {
		t.Fatalf("failed to pop value from redis queue: %v", err)
	}

	var internalData data.InternalData
	err = msgpack.Unmarshal([]byte(pushedCmd), &internalData)
	if err != nil {
		t.Fatalf("failed to unmarshal msgpack data: %v", err)
	}

	// Verify the data
	if internalData.AgentID != agentID {
		t.Errorf("expected agent_id %s, got %s", agentID, internalData.AgentID)
	}
	if internalData.RequestID == "" {
		t.Error("RequestID is empty")
	}
	if internalData.TraceID == "" {
		t.Error("TraceID is empty")
	}
	if internalData.TimestampMs == 0 {
		t.Error("TimestampMs is zero")
	}
	if val, ok := internalData.Payload["key"].(string); !ok || val != "value" {
		t.Errorf("payload data is incorrect")
	}
}

func TestSubmitHandler_WrongMethod(t *testing.T) {
	mr, _ := miniredis.Run()
	defer mr.Close()
	rdb := redis.NewClient(&redis.Options{Addr: mr.Addr()})

	handler := submitHandler(rdb)
	server := httptest.NewServer(handler)
	defer server.Close()

	req, err := http.NewRequest(http.MethodGet, server.URL, nil)
	if err != nil {
		t.Fatalf("failed to create request: %v", err)
	}

	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		t.Fatalf("failed to send request: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusMethodNotAllowed {
		t.Errorf("expected status %d, got %d", http.StatusMethodNotAllowed, resp.StatusCode)
	}
	// Ensure nothing was pushed to Redis
	if mr.Exists(redisQueue) {
		t.Error("a message was pushed to redis on a failed request")
	}
}

func TestSubmitHandler_BadJSON(t *testing.T) {
	mr, _ := miniredis.Run()
	defer mr.Close()
	rdb := redis.NewClient(&redis.Options{Addr: mr.Addr()})

	handler := submitHandler(rdb)
	server := httptest.NewServer(handler)
	defer server.Close()

	body := `{"agent_id": "test-agent", "data":`
	req, err := http.NewRequest(http.MethodPost, server.URL, strings.NewReader(body))
	if err != nil {
		t.Fatalf("failed to create request: %v", err)
	}

	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		t.Fatalf("failed to send request: %v", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusBadRequest {
		t.Errorf("expected status %d, got %d", http.StatusBadRequest, resp.StatusCode)
	}
	// Ensure nothing was pushed to Redis
	if mr.Exists(redisQueue) {
		t.Error("a message was pushed to redis on a failed request")
	}
}
