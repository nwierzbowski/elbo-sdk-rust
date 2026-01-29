use pivot_com_types::Buffer;
use pivot_com_types::EngineCommand;
use pivot_com_types::EngineResponse;
use pivot_com_types::OP_DROP_GROUPS;
use pivot_com_types::OP_GET_SURFACE_TYPES;
use pivot_com_types::OP_ORGANIZE_OBJECTS;
use pivot_com_types::OP_SET_SURFACE_TYPES;
use pivot_com_types::OP_STANDARDIZE_GROUPS;
use pivot_com_types::OP_STANDARDIZE_SYNCED_GROUPS;
use pivot_com_types::asset_names::GroupNames;
use pivot_com_types::asset_ptr::AssetPtr;
use pivot_com_types::asset_surface::GroupSurface;

use crate::asset_data_slices::AssetDataSlices;
use crate::engine_client::EngineClient;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};


pub static CLIENT: LazyLock<EngineClient> = LazyLock::new(|| EngineClient::new());
pub static ENGINE_DIR: LazyLock<Mutex<Option<PathBuf>>> = LazyLock::new(|| Mutex::new(None));


pub fn start_engine() -> Result<(), String> {
    let engine_path = resolve_engine_binary_path()
        .ok_or_else(|| "Failed to locate pivot_engine binary".to_string())?;
    CLIENT.start(engine_path.to_string_lossy().to_string())?;
    Ok(())
}

pub fn stop_engine() -> Result<(), String> {
    CLIENT.stop()?;
    Ok(())
}

pub fn poll_mesh_sync() -> Result<Option<Vec<AssetDataSlices>>, String> {
    let mp = match CLIENT.poll_mesh_sync() {
        Ok(Some(mp)) => mp,
        Ok(None) => return Ok(None),
        Err(e) => return Err(e),
    };

    let asset_ptrs = mp.inline_data.to_asset_meta_ptr(mp.num_groups as usize);
    let mut asset_slices = Vec::new();

    for (shm, group_metadata) in asset_ptrs {
        let slice = unsafe { AssetDataSlices::new(shm, &*group_metadata)? };
        asset_slices.push(slice);
    }


    Ok(Some(asset_slices))
}

pub fn standardize_groups_command(meta_vec: Vec<AssetPtr>) -> Result<EngineResponse, String> {
    let mut command = EngineCommand {
        should_cache: 1,
        op_id: OP_STANDARDIZE_GROUPS,
        num_groups: meta_vec.len() as u32,
        inline_data: Buffer::new(), 
    };
    
    command.inline_data.copy_payload(&meta_vec);
    
    CLIENT.send_command(command)
}

pub fn standardize_synced_groups_command(
    group_names: Vec<String>,
    surface_types: Vec<u32>,
) -> Result<EngineResponse, String> {
    // let command = json!({
    //     "id": COMMAND_CLASSIFY_GROUPS,
    //     "op": "standardize_synced_groups",
    //     "group_names": group_names,
    //     "surface_contexts": surface_contexts,
    // });
    let count = group_names.len() as u32;
    let mut surface_vec: Vec<GroupSurface> = Vec::with_capacity(count as usize);

    for i in 0..count as usize {
        let surf = GroupSurface::new(&group_names[i], surface_types[i] as u64);
        surface_vec.push(surf);
    }

    let mut command = EngineCommand {
        should_cache: 1,
        op_id: OP_STANDARDIZE_SYNCED_GROUPS,
        num_groups: count,
        inline_data: Buffer::new(),
    };

    command.inline_data.copy_payload(&surface_vec);

    CLIENT.send_command(command)
}

pub fn set_surface_types_command(
    group_surface_map: HashMap<String, i64>,
) -> Result<EngineResponse, String> {
    let count = group_surface_map.len() as u32;
    let mut surface_vec: Vec<GroupSurface> = Vec::with_capacity(count as usize);

    group_surface_map.iter().for_each(|(name, surface_type)| {
        let surf = GroupSurface::new(name, *surface_type as u64);
        surface_vec.push(surf);
    });

    // let command = json!({
    //     "id": COMMAND_SET_SURFACE_TYPES,
    //     "op": "set_surface_types",
    //     "classifications": classifications
    // });

    let mut command = EngineCommand {
        should_cache: 1,
        op_id: OP_SET_SURFACE_TYPES,
        num_groups: count,
        inline_data: Buffer::new(),
    };

    command.inline_data.copy_payload(&surface_vec);

    CLIENT.send_command(command)
}

pub fn drop_groups_command(group_names: Vec<String>) -> Result<EngineResponse, String> {

    // let command = json!({
    //     "id": COMMAND_DROP_GROUPS,
    //     "op": "drop_groups",
    //     "group_names": group_names
    // });
    let count = group_names.len() as u32;
    let mut name_vec: Vec<GroupNames> = Vec::with_capacity(count as usize);

    group_names.iter().for_each(|name| {
        name_vec.push(GroupNames::new(name));
    });

    let mut command = EngineCommand {
        should_cache: 1,
        op_id: OP_DROP_GROUPS,
        num_groups: count,
        inline_data: Buffer::new(),
    };

    command.inline_data.copy_payload(&name_vec);

    CLIENT.send_command(command)
}

pub fn organize_objects_command() -> Result<EngineResponse, String> {
    // let command = json!({
    //     "id": COMMAND_ORGANIZE_OBJECTS,
    //     "op": "organize_objects"
    // });

    let command = EngineCommand {
        should_cache: 1,
        op_id: OP_ORGANIZE_OBJECTS,
        num_groups: 0,
        inline_data: Buffer::new(),
    };

    CLIENT.send_command(command)
}

pub fn get_surface_types_command() -> Result<EngineResponse, String> {
    // let command = json!({
    //     "id": COMMAND_GET_GROUP_SURFACE_TYPES,
    //     "op": "get_surface_types"
    // });

    let command = EngineCommand {
        should_cache: 1,
        op_id: OP_GET_SURFACE_TYPES,
        num_groups: 0,
        inline_data: Buffer::new(),
    };

    CLIENT.send_command(command)
}

pub fn set_engine_dir(path: PathBuf) {
    let mut guard = ENGINE_DIR
        .lock()
        .expect("Failed to lock static engine directory");
    *guard = Some(path);
}

fn stored_engine_dir() -> Option<PathBuf> {
    ENGINE_DIR.lock().ok().and_then(|guard| guard.clone())
}

fn pivot_engine_executable_name() -> &'static str {
    if cfg!(target_os = "windows") {
        "pivot_engine.exe"
    } else {
        "pivot_engine"
    }
}

fn ensure_executable(path: &PathBuf) {
    #[cfg(unix)]
    {
        if let Ok(meta) = fs::metadata(path) {
            let mode = meta.permissions().mode();
            if mode & 0o111 == 0 {
                let _ = fs::set_permissions(path, fs::Permissions::from_mode(mode | 0o111));
            }
        }
    }
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
                // return Some(pb);
                ensure_executable(&pb);
                return Some(pb);
            }
        }
    }

    if let Some(engine_dir) = stored_engine_dir() {
        let exe_name = pivot_engine_executable_name();
        let platform_path = engine_dir.join(get_platform_id()).join(exe_name);
        if platform_path.is_file() {
            // return Some(platform_path);
            ensure_executable(&platform_path);
            return Some(platform_path);
        }

        let fallback = engine_dir.join(exe_name);
        if fallback.is_file() {
            // return Some(fallback);
            ensure_executable(&fallback);
            return Some(fallback);
        }
    }

    None
}
