use crate::engine_client::EngineClient;
use serde_json::json;
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;
use std::sync::LazyLock;
use tokio::runtime::Runtime;

const COMMAND_SET_SURFACE_TYPES: i64 = 4;
const COMMAND_DROP_GROUPS: i64 = 5;
const COMMAND_CLASSIFY_GROUPS: i64 = 1;
const COMMAND_CLASSIFY_OBJECTS: i64 = 1;
const COMMAND_GET_GROUP_SURFACE_TYPES: i64 = 2;

#[derive(Debug)]
pub struct EngineApi {
    client: EngineClient,
}

impl EngineApi {
    pub fn new(client: EngineClient) -> Self {
        EngineApi { client }
    }

    pub async fn start(&self, path: String) -> Result<(), String> {
        self.client.start(path).await
    }

    pub async fn stop(&self) -> Result<(), String> {
        self.client.stop().await
    }

    pub async fn standardize_groups_command(
        &self,
        verts_shm_name: String,
        edges_shm_name: String,
        rotations_shm_name: String,
        scales_shm_name: String,
        offsets_shm_name: String,
        vert_counts: Vec<i64>,
        edge_counts: Vec<i64>,
        object_counts: Vec<i64>,
        group_names: Vec<String>,
        surface_contexts: Vec<String>,
    ) -> Result<String, String> {
        let command = json!({
            "id": COMMAND_CLASSIFY_GROUPS,
            "op": "standardize_groups",
            "shm_verts": verts_shm_name,
            "shm_edges": edges_shm_name,
            "shm_rotations": rotations_shm_name,
            "shm_scales": scales_shm_name,
            "shm_offsets": offsets_shm_name,
            "vert_counts": vert_counts,
            "edge_counts": edge_counts,
            "object_counts": object_counts,
            "group_names": group_names,
            "surface_contexts": surface_contexts,
        });

        self.client.send_command(command.to_string()).await
    }

    pub async fn standardize_synced_groups_command(
        &self,
        group_names: Vec<String>,
        surface_contexts: Vec<String>,
    ) -> Result<String, String> {
        let command = json!({
            "id": COMMAND_CLASSIFY_GROUPS,
            "op": "standardize_synced_groups",
            "group_names": group_names,
            "surface_contexts": surface_contexts,
        });

        self.client.send_command(command.to_string()).await
    }

    pub async fn standardize_objects_command(
        &self,
        verts_shm_name: String,
        edges_shm_name: String,
        rotations_shm_name: String,
        scales_shm_name: String,
        offsets_shm_name: String,
        vert_counts: Vec<i64>,
        edge_counts: Vec<i64>,
        object_names: Vec<String>,
        surface_contexts: Vec<String>,
    ) -> Result<String, String> {
        let command = json!({
            "id": COMMAND_CLASSIFY_OBJECTS,
            "op": "standardize_objects",
            "shm_verts": verts_shm_name,
            "shm_edges": edges_shm_name,
            "shm_rotations": rotations_shm_name,
            "shm_scales": scales_shm_name,
            "shm_offsets": offsets_shm_name,
            "vert_counts": vert_counts,
            "edge_counts": edge_counts,
            "object_names": object_names,
            "surface_contexts": surface_contexts,
        });

        self.client.send_command(command.to_string()).await
    }

    pub async fn set_surface_types_command(
        &self,
        group_surface_map: HashMap<String, i64>,
    ) -> Result<String, String> {
        if group_surface_map.is_empty() {
            return Ok(json!({"ok": true}).to_string());
        }

        let classifications: Vec<_> = group_surface_map
            .into_iter()
            .map(|(name, surface)| json!({"group_name": name, "surface_type": surface}))
            .collect();

        let command = json!({
            "id": COMMAND_SET_SURFACE_TYPES,
            "op": "set_surface_types",
            "classifications": classifications
        });

        self.client.send_command(command.to_string()).await
    }

    pub async fn drop_groups_command(&self, group_names: Vec<String>) -> Result<String, String> {
        if group_names.is_empty() {
            return Ok(json!({"dropped_count": 0}).to_string());
        }

        let command = json!({
            "id": COMMAND_DROP_GROUPS,
            "op": "drop_groups",
            "group_names": group_names
        });

        self.client.send_command(command.to_string()).await
    }

    pub async fn get_surface_types_command(&self) -> Result<String, String> {
        let command = json!({
            "id": COMMAND_GET_GROUP_SURFACE_TYPES,
            "op": "get_surface_types"
        });

        self.client.send_command(command.to_string()).await
    }

    pub async fn get_license_command(&self) -> Result<String, String> {
        // Mirror the Cython `sync_license_mode` behaviour by asking the engine
        // for its compiled edition via a simple command. The engine may also
        // support a dedicated RPC; if so, this forwards a small JSON request.
        let command = json!({
            "op": "sync_license_mode"
        });

        self.client.send_command(command.to_string()).await
    }
}

pub async fn start_engine(path: String) -> Result<(), String> {
    ENGINE_API.start(path).await
}

pub async fn stop_engine() -> Result<(), String> {
    ENGINE_API.stop().await
}

pub async fn standardize_groups_command(
    verts_shm_name: String,
    edges_shm_name: String,
    rotations_shm_name: String,
    scales_shm_name: String,
    offsets_shm_name: String,
    vert_counts: Vec<i64>,
    edge_counts: Vec<i64>,
    object_counts: Vec<i64>,
    group_names: Vec<String>,
    surface_contexts: Vec<String>,
) -> Result<String, String> {
    ENGINE_API
        .standardize_groups_command(
            verts_shm_name,
            edges_shm_name,
            rotations_shm_name,
            scales_shm_name,
            offsets_shm_name,
            vert_counts,
            edge_counts,
            object_counts,
            group_names,
            surface_contexts,
        )
        .await
}

pub async fn standardize_synced_groups_command(
    group_names: Vec<String>,
    surface_contexts: Vec<String>,
) -> Result<String, String> {
    ENGINE_API
        .standardize_synced_groups_command(group_names, surface_contexts)
        .await
}

pub async fn standardize_objects_command(
    verts_shm_name: String,
    edges_shm_name: String,
    rotations_shm_name: String,
    scales_shm_name: String,
    offsets_shm_name: String,
    vert_counts: Vec<i64>,
    edge_counts: Vec<i64>,
    object_names: Vec<String>,
    surface_contexts: Vec<String>,
) -> Result<String, String> {
    ENGINE_API
        .standardize_objects_command(
            verts_shm_name,
            edges_shm_name,
            rotations_shm_name,
            scales_shm_name,
            offsets_shm_name,
            vert_counts,
            edge_counts,
            object_names,
            surface_contexts,
        )
        .await
}

pub async fn set_surface_types_command(
    group_surface_map: HashMap<String, i64>,
) -> Result<String, String> {
    ENGINE_API
        .set_surface_types_command(group_surface_map)
        .await
}

pub async fn drop_groups_command(group_names: Vec<String>) -> Result<String, String> {
    ENGINE_API.drop_groups_command(group_names).await
}

pub async fn get_surface_types_command() -> Result<String, String> {
    ENGINE_API.get_surface_types_command().await
}

pub async fn get_license_command() -> Result<String, String> {
    ENGINE_API.get_license_command().await
}

// Platform / binary resolution helpers (mirrors C++ `get_platform_id`/`resolve_engine_binary_path`)
pub fn get_platform_id() -> String {
    let system = match env::consts::OS {
        "windows" => "windows",
        "macos" => "macos",
        "linux" => "linux",
        other => other,
    };

    let arch = match env::consts::ARCH {
        "x86_64" => "x86-64",
        "aarch64" => "arm64",
        other => other,
    };

    format!("{}-{}", system, arch)
}

pub fn resolve_engine_binary_path() -> Option<PathBuf> {
    // Respect explicit override if set and valid
    if let Some(val) = env::var_os("PIVOT_ENGINE_PATH") {
        if !val.is_empty() {
            let pb = PathBuf::from(&val);
            if pb.is_file() {
                return Some(pb);
            }
        }
    }

    if let Ok(found) = which::which("pivot_engine") {
        return Some(found);
    }

    None
}

pub static ENGINE_API: LazyLock<EngineApi> = LazyLock::new(|| EngineApi::new(EngineClient::new()));
pub static RUNTIME: LazyLock<Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime")
});
