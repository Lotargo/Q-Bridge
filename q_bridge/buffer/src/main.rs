use prost::Message;
use redis::aio::MultiplexedConnection;
use redis::{AsyncCommands, from_redis_value, Value, streams::StreamReadOptions};
use std::collections::HashMap;
use tracing::{error, info, instrument};
use tracing_subscriber::prelude::*;

// Import the protobuf definitions from the common crate
use common::gateway::InternalRequest;

const REDIS_URL: &str = "redis://127.0.0.1:6379/";
const STREAM_NAME: &str = "q_bridge_stream";
const GROUP_NAME: &str = "q_bridge_group";
const CONSUMER_NAME: &str = "buffer-consumer-1";

// Creates the consumer group. Idempotent.
async fn create_consumer_group(client: &redis::Client) -> redis::RedisResult<()> {
    let mut conn = client.get_multiplexed_async_connection().await?;
    let result: Result<(), _> = conn
        .xgroup_create_mkstream(STREAM_NAME, GROUP_NAME, "0-0")
        .await;
    match result {
        Ok(_) => {
            info!(
                "Consumer group '{}' created for stream '{}'",
                GROUP_NAME, STREAM_NAME
            );
        }
        Err(e) => {
            if e.to_string().contains("BUSYGROUP") {
                info!(
                    "Consumer group '{}' already exists for stream '{}'",
                    GROUP_NAME, STREAM_NAME
                );
            } else {
                return Err(e);
            }
        }
    }
    Ok(())
}

#[instrument(skip(conn))]
async fn process_message(
    message_id: &str,
    payload_bytes: &[u8],
    conn: &mut MultiplexedConnection,
) -> redis::RedisResult<()> {
    match InternalRequest::decode(payload_bytes) {
        Ok(req) => {
            info!(
                request_id = %req.request_id,
                agent_id = %req.agent_id,
                "Processing message"
            );
        }
        Err(e) => {
            error!(message_id = %message_id, "Failed to decode message: {}", e);
        }
    }

    conn.xack::<_, _, _, ()>(STREAM_NAME, GROUP_NAME, &[message_id]).await?;
    info!(message_id = %message_id, "Acknowledged message");

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .with(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    info!("Starting buffer consumer...");
    let client = redis::Client::open(REDIS_URL)?;

    // Create the group using a dedicated connection
    create_consumer_group(&client).await?;

    // Use a multiplexed connection for the main loop, as it's cloneable
    let mut conn = client.get_multiplexed_async_connection().await?;

    info!("Waiting for messages...");
    loop {
        let opts = StreamReadOptions::default().count(1).group(GROUP_NAME, CONSUMER_NAME);

        let response: Result<Value, _> = conn.xread_options(&[STREAM_NAME], &[">"], &opts).await;

        let response = match response {
            Ok(val) => val,
            Err(e) => {
                error!("Error reading from Redis stream: {}", e);
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                continue;
            }
        };

        if let Value::Bulk(streams) = response {
            if streams.is_empty() {
                // No new messages, wait a bit before polling again
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                continue;
            }

            if let Value::Bulk(stream_data) = &streams[0] {
                if let Value::Bulk(messages) = &stream_data[1] {
                    for message in messages {
                        if let Value::Bulk(message_parts) = message {
                            let id: String = from_redis_value(&message_parts[0])?;
                            let fields: HashMap<String, Vec<u8>> =
                                from_redis_value(&message_parts[1])?;
                            if let Some(payload) = fields.get("payload") {
                                // Clone the connection for the processing task
                                if let Err(e) = process_message(&id, payload, &mut conn.clone()).await {
                                    error!("Failed to process message {}: {}", id, e);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
