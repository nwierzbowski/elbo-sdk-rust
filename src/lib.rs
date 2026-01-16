mod engine_api;
mod engine_client; // This line remains unchanged
extern crate iceoryx2_loggers;

use pyo3::prelude::*;

#[pymodule(name = "_elbo_sdk_rust")]
mod elbo_sdk_rust {
    use pyo3::prelude::*;

    use crate::engine_api::RUNTIME;
    use std::path::PathBuf;
    use iceoryx2_bb_container::semantic_string::SemanticString;
    use iceoryx2_bb_posix::shared_memory::{
        CreationMode, Permission, SharedMemory, SharedMemoryBuilder,
    };

    use iceoryx2_bb_system_types::file_name::FileName;
    use pyo3::Py;
    use pyo3::PyAny;
    use pyo3::ffi;
    use rand::Rng;
    use std::os::raw::c_char;

    // Helper function to abstract the common async pattern
    fn run_async<T, E, F>(future: F) -> PyResult<T>
    where
        F: std::future::Future<Output = Result<T, E>> + Send + 'static,
        T: Send,
        E: std::fmt::Display + Send,
    {
        // Run the future in the detached runtime
        let result = Python::attach(|py: Python| py.detach(|| RUNTIME.block_on(future)))
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))?;
        Ok(result)
    }

    #[pyfunction]
    fn start_engine() -> PyResult<()> {
        run_async(crate::engine_api::start_engine())
    }

    // `set_engine_dir` is intentionally not exposed to Python anymore.

    #[pyfunction]
    fn stop_engine() -> PyResult<()> {
        run_async(crate::engine_api::stop_engine())
    }

    #[pyfunction]
    fn standardize_synced_groups_command(
        group_names: Vec<String>,
        surface_contexts: Vec<String>,
    ) -> PyResult<String> {
        run_async(crate::engine_api::standardize_synced_groups_command(
            group_names,
            surface_contexts,
        ))
    }

    #[pyfunction]
    fn set_surface_types_command(
        group_surface_map: std::collections::HashMap<String, i64>,
    ) -> PyResult<String> {
        run_async(crate::engine_api::set_surface_types_command(
            group_surface_map,
        ))
    }

    #[pyfunction]
    fn drop_groups_command(group_names: Vec<String>) -> PyResult<String> {
        run_async(crate::engine_api::drop_groups_command(group_names))
    }

    #[pyfunction]
    fn get_surface_types_command() -> PyResult<String> {
        run_async(crate::engine_api::get_surface_types_command())
    }

    #[pyfunction]
    fn organize_objects_command() -> PyResult<String> {
        let result = run_async(crate::engine_api::organize_objects_command())?;
        Ok(result)
    }

    #[pyfunction]
    fn get_license_command() -> PyResult<String> {
        let result: String = run_async(crate::engine_api::get_license_command())?;
        let v: serde_json::Value = serde_json::from_str(&result)
            .map_err(|e| pyo3::exceptions::PyValueError::new_err(format!("invalid JSON: {}", e)))?;
        let engine_edition = v
            .get("engine_edition")
            .and_then(|val| val.as_str())
            .map(|s| s.to_string());
        Ok(engine_edition.unwrap_or_default())
    }

    #[pyfunction]
    fn get_platform_id() -> PyResult<String> {
        Ok(crate::engine_api::get_platform_id())
    }

    #[pyfunction]
    fn set_engine_dir(path: String) -> PyResult<()> {
        crate::engine_api::set_engine_dir(PathBuf::from(path));
        Ok(())
    }

    struct StandardizeSharedMemory {
        verts: SharedMemory,
        edges: SharedMemory,
        rotations: SharedMemory,
        scales: SharedMemory,
        offsets: SharedMemory,
        verts_name: String,
        edges_name: String,
        rotations_name: String,
        scales_name: String,
        offsets_name: String,
    }

    impl StandardizeSharedMemory {
        fn names(&self) -> (String, String, String, String, String) {
            (
                self.verts_name.clone(),
                self.edges_name.clone(),
                self.rotations_name.clone(),
                self.scales_name.clone(),
                self.offsets_name.clone(),
            )
        }

        fn buffers(
            &mut self,
            py: Python,
        ) -> PyResult<(Py<PyAny>, Py<PyAny>, Py<PyAny>, Py<PyAny>, Py<PyAny>)> {
            let verts_mv = memoryview_from_shm(py, &mut self.verts)?;
            let edges_mv = memoryview_from_shm(py, &mut self.edges)?;
            let rotations_mv = memoryview_from_shm(py, &mut self.rotations)?;
            let scales_mv = memoryview_from_shm(py, &mut self.scales)?;
            let offsets_mv = memoryview_from_shm(py, &mut self.offsets)?;
            Ok((verts_mv, edges_mv, rotations_mv, scales_mv, offsets_mv))
        }
    }

    #[pyclass(unsendable)]
    struct StandardizeObjectContext {
        shared: StandardizeSharedMemory,
        vert_counts: Vec<i64>,
        edge_counts: Vec<i64>,
        object_names: Vec<String>,
        surface_contexts: Vec<String>,
    }

    #[pymethods]
    impl StandardizeObjectContext {
        fn buffers(
            &mut self,
            py: Python,
        ) -> PyResult<(Py<PyAny>, Py<PyAny>, Py<PyAny>, Py<PyAny>, Py<PyAny>)> {
            self.shared.buffers(py)
        }

        fn finalize(&mut self) -> PyResult<String> {
            let (verts_name, edges_name, rotations_name, scales_name, offsets_name) =
                self.shared.names();
            let vert_counts = self.vert_counts.clone();
            let edge_counts = self.edge_counts.clone();
            let object_names = self.object_names.clone();
            let surface_contexts = self.surface_contexts.clone();
            run_async(crate::engine_api::standardize_objects_command(
                verts_name,
                edges_name,
                rotations_name,
                scales_name,
                offsets_name,
                vert_counts,
                edge_counts,
                object_names,
                surface_contexts,
            ))
        }
    }

    #[pyclass(unsendable)]
    struct StandardizeGroupContext {
        shared: StandardizeSharedMemory,
        vert_counts: Vec<i64>,
        edge_counts: Vec<i64>,
        object_counts: Vec<i64>,
        group_names: Vec<String>,
        surface_contexts: Vec<String>,
    }

    #[pymethods]
    impl StandardizeGroupContext {
        fn buffers(
            &mut self,
            py: Python,
        ) -> PyResult<(Py<PyAny>, Py<PyAny>, Py<PyAny>, Py<PyAny>, Py<PyAny>)> {
            self.shared.buffers(py)
        }

        fn finalize(&mut self) -> PyResult<String> {
            let (verts_name, edges_name, rotations_name, scales_name, offsets_name) =
                self.shared.names();
            let vert_counts = self.vert_counts.clone();
            let edge_counts = self.edge_counts.clone();
            let object_counts = self.object_counts.clone();
            let group_names = self.group_names.clone();
            let surface_contexts = self.surface_contexts.clone();
            run_async(crate::engine_api::standardize_groups_command(
                verts_name,
                edges_name,
                rotations_name,
                scales_name,
                offsets_name,
                vert_counts,
                edge_counts,
                object_counts,
                group_names,
                surface_contexts,
            ))
        }
    }

    fn build_standardize_shared_memory(
        total_verts: u32,
        total_edges: u32,
        total_objects: u32,
    ) -> PyResult<StandardizeSharedMemory> {
        let verts_size = (total_verts as usize) * 3 * 4;
        let edges_size = (total_edges as usize) * 2 * 4;
        let rotations_size = (total_objects as usize) * 4 * 4;
        let scales_size = (total_objects as usize) * 3 * 4;
        let offsets_size = (total_objects as usize) * 3 * 4;

        let uid = new_uid16();
        let verts_name = format!("sp_v_{}", uid);
        let edges_name = format!("sp_e_{}", uid);
        let rotations_name = format!("sp_r_{}", uid);
        let scales_name = format!("sp_s_{}", uid);
        let offsets_name = format!("sp_o_{}", uid);

        let verts = create_shm_segment(&verts_name, verts_size)?;
        let edges = create_shm_segment(&edges_name, edges_size)?;
        let rotations = create_shm_segment(&rotations_name, rotations_size)?;
        let scales = create_shm_segment(&scales_name, scales_size)?;
        let offsets = create_shm_segment(&offsets_name, offsets_size)?;

        Ok(StandardizeSharedMemory {
            verts,
            edges,
            rotations,
            scales,
            offsets,
            verts_name,
            edges_name,
            rotations_name,
            scales_name,
            offsets_name,
        })
    }

    #[pyfunction]
    fn prepare_standardize_objects(
        total_verts: u32,
        total_edges: u32,
        total_objects: u32,
        vert_counts: Vec<i64>,
        edge_counts: Vec<i64>,
        object_names: Vec<String>,
        surface_contexts: Vec<String>,
    ) -> PyResult<StandardizeObjectContext> {
        let shared = build_standardize_shared_memory(total_verts, total_edges, total_objects)?;
        Ok(StandardizeObjectContext {
            shared,
            vert_counts,
            edge_counts,
            object_names,
            surface_contexts,
        })
    }

    #[pyfunction]
    fn prepare_standardize_groups(
        total_verts: u32,
        total_edges: u32,
        total_objects: u32,
        vert_counts: Vec<i64>,
        edge_counts: Vec<i64>,
        object_counts: Vec<i64>,
        group_names: Vec<String>,
        surface_contexts: Vec<String>,
    ) -> PyResult<StandardizeGroupContext> {
        let shared = build_standardize_shared_memory(total_verts, total_edges, total_objects)?;
        Ok(StandardizeGroupContext {
            shared,
            vert_counts,
            edge_counts,
            object_counts,
            group_names,
            surface_contexts,
        })
    }

    fn new_uid16() -> String {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut rng = rand::thread_rng();
        let mut out = String::with_capacity(16);
        for _ in 0..16 {
            let idx = rng.gen_range(0..16);
            out.push(HEX[idx] as char);
        }
        out
    }

    fn create_shm_segment(name: &str, size: usize) -> PyResult<SharedMemory> {
        let file_name = FileName::new(name.as_bytes()).map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "invalid shared memory name '{}': {:?}",
                name, e
            ))
        })?;

        SharedMemoryBuilder::new(&file_name)
            .is_memory_locked(false)
            .creation_mode(CreationMode::PurgeAndCreate)
            .size(size)
            .permission(Permission::OWNER_ALL)
            .zero_memory(true)
            .create()
            .map_err(|e| {
                PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                    "failed to create shared memory '{}': {:?}",
                    name, e
                ))
            })
    }

    fn memoryview_from_shm(py: Python, shm: &mut SharedMemory) -> PyResult<Py<PyAny>> {
        let slice = shm.as_mut_slice();
        let ptr = slice.as_mut_ptr() as *mut c_char;
        let len = slice.len() as isize;
        let mv = unsafe { ffi::PyMemoryView_FromMemory(ptr, len, ffi::PyBUF_WRITE) };
        if mv.is_null() {
            return Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                "failed to create memoryview for shared memory",
            ));
        }
        Ok(unsafe { Py::<PyAny>::from_owned_ptr(py, mv) })
    }
}
