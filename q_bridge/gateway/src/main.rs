use actix_web::{web, App, HttpResponse, HttpServer, Responder};
use prost::Message;
use redis::AsyncCommands;
use tonic::{transport::Server, Request, Response, Status};
use tracing::{error, info, instrument};
use tracing_subscriber::prelude::*;
use uuid::Uuid;

// Import the shared protobuf definitions from the `common` crate
use common::gateway::{
    gateway_service_server::{GatewayService, GatewayServiceServer},
    InternalRequest, SubmitRequestResponse,
};

const REDIS_URL: &str = "redis://127.0.0.1:6379/";
const STREAM_NAME: &str = "q_bridge_stream";

// Shared application state, designed to be cloned
#[derive(Clone)]
struct AppState {
    redis_client: redis::Client,
}

// Helper function containing the core logic for processing a request
#[instrument(skip(redis_client, internal_req))]
async fn process_request(
    mut internal_req: InternalRequest,
    redis_client: &redis::Client,
) -> Result<SubmitRequestResponse, Status> {
    if internal_req.request_id.is_empty() {
        internal_req.request_id = Uuid::new_v4().to_string();
    }
    let request_id = internal_req.request_id.clone();
    info!(request_id = %request_id, "Processing request");

    let mut conn = redis_client
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| {
            error!("Failed to get Redis connection: {}", e);
            Status::internal("Failed to connect to buffer")
        })?;

    let mut buf = Vec::new();
    internal_req.encode(&mut buf).map_err(|e| {
        error!("Failed to encode protobuf message: {}", e);
        Status::internal("Failed to serialize request")
    })?;

    let _: () = conn
        .xadd(STREAM_NAME, "*", &[("payload", &buf)])
        .await
        .map_err(|e| {
            error!("Failed to add message to Redis stream: {}", e);
            Status::internal("Failed to write to buffer")
        })?;

    info!(request_id = %request_id, "Request accepted and buffered");

    Ok(SubmitRequestResponse {
        request_id,
        status: "accepted".to_string(),
    })
}

// The gRPC service implementation
struct MyGatewayService {
    state: AppState,
}

#[tonic::async_trait]
impl GatewayService for MyGatewayService {
    #[instrument(skip(self, request))]
    async fn submit_request(
        &self,
        request: Request<InternalRequest>,
    ) -> Result<Response<SubmitRequestResponse>, Status> {
        let req = request.into_inner();
        let result = process_request(req, &self.state.redis_client).await?;
        Ok(Response::new(result))
    }
}

// The Actix-Web REST endpoint handler
#[instrument(skip(state, payload))]
async fn rest_submit_request(
    state: web::Data<AppState>,
    payload: web::Json<serde_json::Value>,
) -> impl Responder {
    let internal_req = InternalRequest {
        request_id: Uuid::new_v4().to_string(),
        agent_id: payload
            .get("agent_id")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string(),
        payload: payload
            .get("payload")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .as_bytes()
            .to_vec(),
        metadata: Default::default(),
    };

    match process_request(internal_req, &state.redis_client).await {
        Ok(response) => HttpResponse::Accepted().json(response),
        Err(status) => {
            error!("Failed to process REST request: {}", status);
            HttpResponse::InternalServerError().body(status.message().to_string())
        }
    }
}

// A simple health check endpoint for Actix-Web
async fn health_check() -> impl Responder {
    HttpResponse::Ok().body("OK")
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup tracing
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    // Initialize shared state
    let app_state = AppState {
        redis_client: redis::Client::open(REDIS_URL)?,
    };
    // Wrap state for Actix-Web
    let web_data = web::Data::new(app_state.clone());

    // --- gRPC Server Task ---
    let grpc_addr = "127.0.0.1:50051".parse()?;
    let grpc_service = MyGatewayService { state: app_state };
    let grpc_server = Server::builder().add_service(GatewayServiceServer::new(grpc_service));
    info!("gRPC server listening on {}", grpc_addr);
    let grpc_handle = tokio::spawn(grpc_server.serve(grpc_addr));

    // --- REST Server Task (Actix-Web) ---
    let rest_addr = "127.0.0.1:3000";
    info!("REST server listening on {}", rest_addr);
    let rest_server = HttpServer::new(move || {
        App::new()
            .app_data(web_data.clone())
            .route("/submit", web::post().to(rest_submit_request))
            .route("/health", web::get().to(health_check))
    })
    .bind(rest_addr)?
    .run();

    let rest_handle = tokio::spawn(rest_server);

    // Wait for both servers to complete
    let (grpc_res, rest_res) = tokio::join!(grpc_handle, rest_handle);
    grpc_res??;
    rest_res??;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use redis::Commands;

    // Helper to get a sync redis connection for test verification
    fn get_sync_redis_connection() -> redis::Connection {
        redis::Client::open(REDIS_URL)
            .expect("Failed to create Redis client for test")
            .get_connection()
            .expect("Failed to get sync Redis connection for test")
    }

    #[tokio::test]
    #[ignore] // This test requires a running Redis instance and should be run with `cargo test -- --ignored`
    async fn test_process_request_success() {
        let redis_client = redis::Client::open(REDIS_URL).unwrap();
        let mut conn = get_sync_redis_connection();

        // Clean up before test
        let _: Result<(), _> = conn.del(STREAM_NAME);

        let request = InternalRequest {
            request_id: "test-req-123".to_string(),
            agent_id: "test-agent".to_string(),
            payload: b"hello world".to_vec(),
            metadata: Default::default(),
        };

        // Call the function under test
        let result = process_request(request, &redis_client).await;
        assert!(result.is_ok());

        let response = result.unwrap();
        assert_eq!(response.request_id, "test-req-123");
        assert_eq!(response.status, "accepted");

        // --- Verification ---
        // 1. Check if the stream length is 1
        let len: i64 = conn.xlen(STREAM_NAME).unwrap();
        assert_eq!(len, 1);

        // 2. Get the last message from the stream and verify its content
        let result: redis::Value = conn.xrange_count(STREAM_NAME, "+", "-", 1).unwrap();
        if let redis::Value::Bulk(messages) = result {
            assert_eq!(messages.len(), 1, "Expected one message in XRANGE response");
            let last_message = &messages[0];

            if let redis::Value::Bulk(message_parts) = last_message {
                // message_parts[0] is the ID, message_parts[1] is the fields
                if let redis::Value::Bulk(fields) = &message_parts[1] {
                    // fields[0] is key "payload", fields[1] is value
                    if let redis::Value::Data(payload_bytes) = &fields[1] {
                        let req: InternalRequest = InternalRequest::decode(payload_bytes.as_slice()).unwrap();
                        assert_eq!(req.request_id, "test-req-123");
                        assert_eq!(req.payload, b"hello world".to_vec());
                    } else {
                        panic!("Expected payload to be redis::Value::Data");
                    }
                } else {
                    panic!("Expected message fields to be redis::Value::Bulk");
                }
            } else {
                panic!("Expected message parts to be redis::Value::Bulk");
            }
        } else {
            panic!("Expected XRANGE response to be redis::Value::Bulk");
        }

        // Clean up after test
        let _: () = conn.del(STREAM_NAME).unwrap();
    }

    #[actix_web::test]
    async fn test_health_check_endpoint() {
        let app = actix_web::test::init_service(
            App::new().route("/health", web::get().to(health_check))
        ).await;
        let req = actix_web::test::TestRequest::get().uri("/health").to_request();
        let resp = actix_web::test::call_service(&app, req).await;
        assert!(resp.status().is_success());

        let body = actix_web::test::read_body(resp).await;
        assert_eq!(body, "OK");
    }
}
