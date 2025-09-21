use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, error};
use warp::Filter;

use crate::storage::Storage;

// Simple TrainDB node with HTTP API
pub struct SimpleTrainDBNode {
    storage: Arc<Mutex<Box<dyn Storage>>>,
    port: u16,
}

impl SimpleTrainDBNode {
    pub fn new(storage: Box<dyn Storage>, port: u16) -> Self {
        Self {
            storage: Arc::new(Mutex::new(storage)),
            port,
        }
    }
    
    pub async fn run(self) -> Result<()> {
        info!("Starting TrainDB HTTP API server on port {}", self.port);
        
        let storage = self.storage.clone();
        
        // GET /api/keys - List all keys
        let list_keys = warp::path("api")
            .and(warp::path("keys"))
            .and(warp::get())
            .and(with_storage(storage.clone()))
            .and_then(handle_list_keys);
        
        // GET /api/keys/{key} - Get a specific key
        let get_key = warp::path("api")
            .and(warp::path("keys"))
            .and(warp::path::param::<String>())
            .and(warp::get())
            .and(with_storage(storage.clone()))
            .and_then(handle_get_key);
        
        // POST /api/keys - Set a key-value pair
        let set_key = warp::path("api")
            .and(warp::path("keys"))
            .and(warp::post())
            .and(warp::body::json())
            .and(with_storage(storage.clone()))
            .and_then(handle_set_key);
        
        // DELETE /api/keys/{key} - Delete a key
        let delete_key = warp::path("api")
            .and(warp::path("keys"))
            .and(warp::path::param::<String>())
            .and(warp::delete())
            .and(with_storage(storage.clone()))
            .and_then(handle_delete_key);
        
        // GET /api/info - Get node information
        let get_info = warp::path("api")
            .and(warp::path("info"))
            .and(warp::get())
            .and(with_storage(storage.clone()))
            .and_then(handle_get_info);
        
        // CORS support
        let cors = warp::cors()
            .allow_any_origin()
            .allow_headers(vec!["content-type"])
            .allow_methods(vec!["GET", "POST", "DELETE", "OPTIONS"]);
        
        let routes = list_keys
            .or(get_key)
            .or(set_key)
            .or(delete_key)
            .or(get_info)
            .with(cors);
        
        info!("TrainDB API available at:");
        info!("  GET    http://localhost:{}/api/keys", self.port);
        info!("  GET    http://localhost:{}/api/keys/{{key}}", self.port);
        info!("  POST   http://localhost:{}/api/keys", self.port);
        info!("  DELETE http://localhost:{}/api/keys/{{key}}", self.port);
        info!("  GET    http://localhost:{}/api/info", self.port);
        
        warp::serve(routes)
            .run(([0, 0, 0, 0], self.port))
            .await;
        
        Ok(())
    }
}

// Helper function to pass storage to handlers
fn with_storage(
    storage: Arc<Mutex<Box<dyn Storage>>>,
) -> impl Filter<Extract = (Arc<Mutex<Box<dyn Storage>>>,), Error = std::convert::Infallible> + Clone {
    warp::any().map(move || storage.clone())
}

// API Request/Response types
#[derive(serde::Deserialize)]
struct SetKeyRequest {
    key: String,
    value: String,
}

#[derive(serde::Serialize)]
struct KeyValueResponse {
    key: String,
    value: String,
}

#[derive(serde::Serialize)]
struct InfoResponse {
    node_type: String,
    version: String,
    key_count: usize,
    size_bytes: u64,
}

// API Handlers
async fn handle_list_keys(
    storage: Arc<Mutex<Box<dyn Storage>>>,
) -> Result<warp::reply::Json, warp::Rejection> {
    let storage = storage.lock().await;
    match storage.list_keys() {
        Ok(keys) => {
            let response: Vec<KeyValueResponse> = keys
                .into_iter()
                .filter_map(|key| {
                    storage.get(&key).ok().flatten().map(|value| KeyValueResponse {
                        key,
                        value,
                    })
                })
                .collect();
            Ok(warp::reply::json(&response))
        }
        Err(e) => {
            error!("Failed to list keys: {}", e);
            Ok(warp::reply::json(&serde_json::json!({"error": e.to_string()})))
        }
    }
}

async fn handle_get_key(
    key: String,
    storage: Arc<Mutex<Box<dyn Storage>>>,
) -> Result<warp::reply::Json, warp::Rejection> {
    let storage = storage.lock().await;
    match storage.get(&key) {
        Ok(Some(value)) => Ok(warp::reply::json(&KeyValueResponse { key, value })),
        Ok(None) => Ok(warp::reply::json(&serde_json::json!({"error": "Key not found"}))),
        Err(e) => {
            error!("Failed to get key: {}", e);
            Ok(warp::reply::json(&serde_json::json!({"error": e.to_string()})))
        }
    }
}

async fn handle_set_key(
    request: SetKeyRequest,
    storage: Arc<Mutex<Box<dyn Storage>>>,
) -> Result<warp::reply::Json, warp::Rejection> {
    let storage = storage.lock().await;
    match storage.set(&request.key, &request.value) {
        Ok(()) => {
            info!("Set key '{}' = '{}'", request.key, request.value);
            Ok(warp::reply::json(&KeyValueResponse {
                key: request.key,
                value: request.value,
            }))
        }
        Err(e) => {
            error!("Failed to set key: {}", e);
            Ok(warp::reply::json(&serde_json::json!({"error": e.to_string()})))
        }
    }
}

async fn handle_delete_key(
    key: String,
    storage: Arc<Mutex<Box<dyn Storage>>>,
) -> Result<warp::reply::Json, warp::Rejection> {
    let storage = storage.lock().await;
    match storage.delete(&key) {
        Ok(()) => {
            info!("Deleted key '{}'", key);
            Ok(warp::reply::json(&serde_json::json!({"message": "Key deleted"})))
        }
        Err(e) => {
            error!("Failed to delete key: {}", e);
            Ok(warp::reply::json(&serde_json::json!({"error": e.to_string()})))
        }
    }
}

async fn handle_get_info(
    storage: Arc<Mutex<Box<dyn Storage>>>,
) -> Result<warp::reply::Json, warp::Rejection> {
    let storage = storage.lock().await;
    match storage.info() {
        Ok(info) => Ok(warp::reply::json(&InfoResponse {
            node_type: "SimpleTrainDB".to_string(),
            version: "1.0.0".to_string(),
            key_count: info.key_count,
            size_bytes: info.size_bytes,
        })),
        Err(e) => {
            error!("Failed to get info: {}", e);
            Ok(warp::reply::json(&serde_json::json!({"error": e.to_string()})))
        }
    }
}