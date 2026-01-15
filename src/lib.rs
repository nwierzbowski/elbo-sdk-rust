use pyo3::prelude::*;
mod engine_client;
mod engine_api; // This line remains unchanged

#[pymodule]
mod elbo_sdk_rust {
    use pyo3::prelude::*;

    // Engine-related functions in a submodule
    #[pymodule]
    mod engine {
        use pyo3::prelude::*;
        use crate::engine_api::RUNTIME;

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
        fn start_engine(path: String) -> PyResult<()> {
            run_async(crate::engine_api::start_engine(path))
        }

        #[pyfunction]
        fn stop_engine() -> PyResult<()> {
            run_async(crate::engine_api::stop_engine())
        }

        #[pyfunction]
        fn standardize_groups_command(
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
        ) -> PyResult<String> {
            run_async(crate::engine_api::standardize_groups_command(
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
            ))
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
        fn standardize_objects_command(
            verts_shm_name: String,
            edges_shm_name: String,
            rotations_shm_name: String,
            scales_shm_name: String,
            offsets_shm_name: String,
            vert_counts: Vec<i64>,
            edge_counts: Vec<i64>,
            object_names: Vec<String>,
            surface_contexts: Vec<String>,
        ) -> PyResult<String> {
            run_async(crate::engine_api::standardize_objects_command(
                verts_shm_name,
                edges_shm_name,
                rotations_shm_name,
                scales_shm_name,
                offsets_shm_name,
                vert_counts,
                edge_counts,
                object_names,
                surface_contexts,
            ))
        }

        #[pyfunction]
        fn set_surface_types_command(
            group_surface_map: std::collections::HashMap<String, i64>,
        ) -> PyResult<String> {
            run_async(crate::engine_api::set_surface_types_command(group_surface_map))
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
        fn get_license_command() -> PyResult<String> {
            run_async(crate::engine_api::get_license_command())
        }

        #[pyfunction]
        fn get_platform_id() -> PyResult<String> {
            Ok(crate::engine_api::get_platform_id())
        }

        #[pyfunction]
        fn get_engine_binary_path() -> PyResult<Option<String>> {
            Ok(crate::engine_api::resolve_engine_binary_path()
                .map(|p| p.to_string_lossy().into_owned()))
        }
    }
}
