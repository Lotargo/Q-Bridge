import subprocess
import time
import requests
import redis
import pyarrow.flight as fl
import uuid
import os
import signal

# --- Configuration ---
REDIS_URL = "redis://localhost:6379"
STREAM_NAME = "q_bridge_stream"
GATEWAY_URL = "http://localhost:3000"
TRANSPORT_URL = "grpc://localhost:50052"

# --- Path Setup ---
SCRIPT_DIR = os.path.dirname(os.path.realpath(__file__))
# The docker-compose file is in the parent directory of this script's directory
DOCKER_COMPOSE_PATH = os.path.join(SCRIPT_DIR, '..', 'docker-compose.yml')
# The root of the workspace is the parent of this script's directory
WORKSPACE_ROOT = os.path.join(SCRIPT_DIR, '..')


# --- Helper Functions ---

def start_redis():
    """Starts Redis using docker-compose."""
    print("--- Starting Redis ---")
    try:
        subprocess.run(["docker", "compose", "-f", DOCKER_COMPOSE_PATH, "up", "-d"], check=True)
        time.sleep(5) # Give Redis time to start
        print("Redis started successfully.")
        r = redis.Redis.from_url(REDIS_URL, decode_responses=True)
        r.ping()
        print("Redis connection successful.")
        return True
    except Exception as e:
        print(f"Error starting or connecting to Redis: {e}")
        return False

def stop_redis():
    """Stops Redis using docker-compose."""
    print("--- Stopping Redis ---")
    subprocess.run(["docker", "compose", "-f", DOCKER_COMPOSE_PATH, "down"], check=True)
    print("Redis stopped.")

def start_service(name):
    """Starts a Rust service from the workspace root and returns its process handle."""
    print(f"--- Starting {name} Service ---")
    log_file_path = os.path.join(SCRIPT_DIR, f"{name}_service.log")
    log_file = open(log_file_path, "w")
    process = subprocess.Popen(
        ["cargo", "run", "--bin", name],
        cwd=WORKSPACE_ROOT, # Run from the root of the q_bridge workspace
        stdout=log_file,
        stderr=subprocess.STDOUT,
        preexec_fn=os.setsid
    )
    time.sleep(5)
    print(f"{name} service started (PID: {process.pid}). Log: {log_file_path}")
    return process, log_file

def stop_service(name, process, log_file):
    """Stops a service process."""
    print(f"--- Stopping {name} Service ---")
    if process.poll() is None:
        os.killpg(os.getpgid(process.pid), signal.SIGTERM)
        process.wait()
        print(f"{name} service stopped.")
    else:
        print(f"{name} service was already stopped.")
    log_file.close()

def clear_redis_stream():
    """Deletes the Redis stream to ensure a clean test run."""
    print("--- Clearing Redis Stream ---")
    try:
        r = redis.Redis.from_url(REDIS_URL)
        r.delete(STREAM_NAME)
        print(f"Stream '{STREAM_NAME}' cleared.")
    except Exception as e:
        print(f"Could not clear Redis stream: {e}")

# --- Test Functions ---

def test_gateway_submission():
    """Tests submitting a request to the gateway."""
    print("\n--- Running Test: Gateway Submission ---")
    test_id = str(uuid.uuid4())
    payload = {"agent_id": "e2e-test-agent", "payload": f"test_data_{test_id}"}

    try:
        response = requests.post(f"{GATEWAY_URL}/submit", json=payload, timeout=10)
        response.raise_for_status()
        print(f"Gateway response: {response.json()}")
        assert response.status_code == 202
        assert "request_id" in response.json()
        print("Gateway submission test PASSED.")
        return test_id
    except Exception as e:
        print(f"Gateway submission test FAILED: {e}")
        return None

def verify_buffer_consumption():
    """Verifies that the buffer service consumed a message."""
    print("\n--- Running Test: Buffer Consumption Verification ---")
    # Give buffer time to process any messages
    time.sleep(5)

    try:
        log_file_path = os.path.join(SCRIPT_DIR, "buffer_service.log")
        with open(log_file_path, "r") as f:
            logs = f.read()

        # Check for the log entry indicating a message was processed
        if "Processing message" in logs:
            print("Buffer consumption verification PASSED.")
            return True
        else:
            print("Buffer consumption verification FAILED: 'Processing message' log not found.")
            print("--- Buffer Logs ---")
            print(logs)
            print("-------------------")
            return False
    except Exception as e:
        print(f"Buffer consumption verification FAILED: {e}")
        return False

def test_transport_service():
    """Tests connecting to the Arrow Flight transport service."""
    print("\n--- Running Test: Transport Service Connection ---")
    try:
        client = fl.connect(TRANSPORT_URL)
        ticket = fl.Ticket(b"test_ticket")
        reader = client.do_get(ticket)
        data = reader.read_all()
        print(f"Transport service returned {data.num_rows} rows. Connection successful.")
        assert data.num_rows == 0
        print("Transport service test PASSED.")
        return True
    except Exception as e:
        print(f"Transport service test FAILED: {e}")
        return False

# --- Main Execution ---

def main():
    """Main test execution orchestrator."""
    services = {}
    success = True
    final_status = 0

    try:
        if not start_redis():
            raise RuntimeError("Failed to start Redis.")

        clear_redis_stream()

        services["gateway"], services["gateway_log"] = start_service("gateway")
        services["buffer"], services["buffer_log"] = start_service("buffer")
        services["transport"], services["transport_log"] = start_service("transport")

        if not test_gateway_submission():
            success = False

        if not verify_buffer_consumption():
            success = False

        if not test_transport_service():
            success = False

    except Exception as e:
        print(f"\nAn unexpected error occurred: {e}")
        success = False
    finally:
        print("\n--- Tearing down services ---")
        for name in ["transport", "buffer", "gateway"]:
            if name in services:
                stop_service(name, services[name], services[f"{name}_log"])

        stop_redis()

        print("\n--- Test Run Summary ---")
        if success:
            print("E2E Test Run: PASSED")
            final_status = 0
        else:
            print("E2E Test Run: FAILED")
            final_status = 1

        exit(final_status)


if __name__ == "__main__":
    main()
