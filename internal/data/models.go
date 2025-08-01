package data

// IncomingPayload represents the structure of the JSON data sent to the /v1/submit endpoint.
type IncomingPayload struct {
	AgentID string                 `json:"agent_id"`
	Data    map[string]interface{} `json:"data"`
}

// InternalData is the final structure that gets serialized to Msgpack and published to Redis.
// It includes metadata enrichment.
type InternalData struct {
	RequestID   string                 `msgpack:"request_id"`
	TraceID     string                 `msgpack:"trace_id"`
	AgentID     string                 `msgpack:"agent_id"`
	Payload     map[string]interface{} `msgpack:"payload"`
	TimestampMs int64                  `msgpack:"timestamp_ms"`
}
