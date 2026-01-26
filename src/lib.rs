mod engine_api;
mod engine_client; // This line remains unchanged
extern crate iceoryx2_loggers;

use pyo3::prelude::*;

#[pymodule(name = "_elbo_sdk_rust")]
mod elbo_sdk_rust {
    use pivot_com_types::MAX_NAME_LEN;
    use pyo3::prelude::*;

    use iceoryx2_bb_posix::shared_memory::SharedMemory;
    use std::path::PathBuf;

    use crate::engine_api;
    use pivot_com_types::com_types;

    use pyo3::Py;
    use pyo3::PyAny;
    use pyo3::ffi;
    use rand::Rng;
    use std::os::raw::c_char;

    #[pyfunction]
    fn start_engine() -> PyResult<()> {
        engine_api::start_engine()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    #[pyfunction]
    fn stop_engine() -> PyResult<()> {
        engine_api::stop_engine()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()))
    }

    #[pyfunction]
    fn standardize_synced_groups_command(
        group_names: Vec<String>,
        surface_contexts: Vec<u32>,
    ) -> () {
        let _ = engine_api::standardize_synced_groups_command(group_names, surface_contexts)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()));
    }

    #[pyfunction]
    fn set_surface_types_command(group_surface_map: std::collections::HashMap<String, i64>) -> () {
        let _ = engine_api::set_surface_types_command(group_surface_map)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()));
    }

    #[pyfunction]
    fn drop_groups_command(group_names: Vec<String>) -> () {
        let _ = engine_api::drop_groups_command(group_names)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()));
    }

    #[pyfunction]
    fn get_surface_types_command() -> () {
        let _ = engine_api::get_surface_types_command()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()));
    }

    #[pyfunction]
    fn organize_objects_command() -> () {
        let _ = engine_api::organize_objects_command()
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()));
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

    #[derive(Debug)]
    struct AssetDataSlices {
        _shm: SharedMemory, // keep backing alive
        uuids: *mut [u8],
        verts: *mut [u8],
        edges: *mut [u8],
        transforms: *mut [u8],
        vert_counts: *mut [u8],
        edge_counts: *mut [u8],
        object_names: *mut [u8],
    }

    impl AssetDataSlices {
        pub fn new(
            shm: SharedMemory,
            group_metadata: &com_types::AssetMeta,
        ) -> Result<Self, String> {
            let base_ptr = shm.base_address().as_ptr() as *mut u8;
            let shm_size = shm.size();
            let shm_slice: &mut [u8] =
                unsafe { std::slice::from_raw_parts_mut(base_ptr, shm_size) };

            let group_name = group_metadata.get_group_name(base_ptr);
            let verts_slice = shm_slice_from_range(
                shm_slice,
                group_metadata.offset_verts,
                (group_metadata.vert_count as usize) * 3 * size_of::<f32>(),
                group_name,
                "verts",
            )?;

            let edges_slice = shm_slice_from_range(
                shm_slice,
                group_metadata.offset_edges,
                (group_metadata.edge_count as usize) * 2 * size_of::<u32>(),
                group_name,
                "edges",
            )?;

            let transforms_slice = shm_slice_from_range(
                shm_slice,
                group_metadata.offset_transforms,
                (group_metadata.object_count as usize) * 16 * size_of::<f32>(),
                group_name,
                "transforms",
            )?;

            //+1 for total at the end
            let vert_counts_slice = shm_slice_from_range(
                shm_slice,
                group_metadata.offset_vert_bases,
                ((group_metadata.object_count) as usize) * size_of::<u32>(),
                group_name,
                "vert_counts",
            )?;

            //+1 for total at the end
            let edge_counts_slice = shm_slice_from_range(
                shm_slice,
                group_metadata.offset_edge_bases,
                ((group_metadata.object_count) as usize) * size_of::<u32>(),
                group_name,
                "edge_counts",
            )?;

            let object_names_slice = shm_slice_from_range(
                shm_slice,
                group_metadata.offset_object_names,
                (group_metadata.object_count as usize) * MAX_NAME_LEN,
                group_name,
                "object_names",
            )?;

            let uuids_slice = shm_slice_from_range(
                shm_slice,
                group_metadata.offset_uuids,
                (group_metadata.object_count as usize) * 16,
                group_name,
                "uuids",
            )?;

            Ok(AssetDataSlices {
                _shm: shm,
                verts: verts_slice,
                edges: edges_slice,
                transforms: transforms_slice,
                vert_counts: vert_counts_slice,
                edge_counts: edge_counts_slice,
                object_names: object_names_slice,
                uuids: uuids_slice,
            })
        }
    }

    #[pyclass(unsendable)]
    struct StandardizeGroupContext {
        pub group_mvs: Vec<AssetDataSlices>,
        pub shm_offset_vec: Vec<com_types::ShmOffset>,
    }

    #[pymethods]
    impl StandardizeGroupContext {
        fn buffers(
            &self,
            py: Python,
            i: usize,
        ) -> PyResult<(
            Py<PyAny>,
            Py<PyAny>,
            Py<PyAny>,
            Py<PyAny>,
            Py<PyAny>,
            Py<PyAny>,
            Py<PyAny>,
        )> {
            let g = self.group_mvs.get(i).ok_or_else(|| {
                PyErr::new::<pyo3::exceptions::PyIndexError, _>(format!("index {} out of range", i))
            })?;

            let v = memoryview_from_slice(py, g.verts)?;
            let e = memoryview_from_slice(py, g.edges)?;
            let t = memoryview_from_slice(py, g.transforms)?;
            let vc = memoryview_from_slice(py, g.vert_counts)?;
            let ec = memoryview_from_slice(py, g.edge_counts)?;
            let on = memoryview_from_slice(py, g.object_names)?;
            let uu = memoryview_from_slice(py, g.uuids)?;

            Ok((v, e, t, vc, ec, on, uu))
        }

        fn finalize(&mut self) -> () {
            let offsets = std::mem::take(&mut self.shm_offset_vec);
            let response = engine_api::standardize_groups_command(offsets)
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()));
            println!("Standardize Groups Response: {:?}", response);
        }
    }

    #[pyfunction]
    fn prepare_standardize_groups(
        vert_counts: Vec<u32>,
        edge_counts: Vec<u32>,
        object_counts: Vec<u32>,
        group_names: Vec<String>,
        surface_contexts: Vec<u16>,
    ) -> PyResult<StandardizeGroupContext> {
        let mut group_mvs = Vec::new();
        let mut shm_offset_vec = Vec::new();

        for i in 0..group_names.len() as usize {
            let (group_metadata, group_mv) = prepare_standardize_group(
                &group_names[i],
                vert_counts[i],
                edge_counts[i],
                object_counts[i],
                surface_contexts[i],
            )
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e))?;

            shm_offset_vec.push(group_metadata);
            group_mvs.push(group_mv);
        }

        Ok(StandardizeGroupContext {
            group_mvs,
            shm_offset_vec,
        })
    }

    fn prepare_standardize_group(
        group_name: &String,
        total_verts: u32,
        total_edges: u32,
        object_count: u32,
        surface_contexts: u16,
    ) -> Result<(com_types::ShmOffset, AssetDataSlices), String> {
        let handle_name = new_uid16();

        let (shm, group_metadata) = com_types::AssetMeta::new(
            total_verts,
            total_edges,
            object_count,
            surface_contexts,
            &group_name,
            &handle_name,
        )
        .map_err(|e| e.to_string())?;

        let shm_offset = com_types::ShmOffset::new(0, handle_name);
        let asset_data_slices =
            AssetDataSlices::new(shm, &group_metadata).map_err(|e| e.to_string())?;

        Ok((shm_offset, asset_data_slices))
    }

    fn shm_slice_from_range(
        shm_slice: &mut [u8],
        offset: u64,
        size: usize,
        group_name: &str,
        label: &str,
    ) -> Result<*mut [u8], String> {
        let total = shm_slice.len();
        let offset = usize::try_from(offset).map_err(|_| {
            format!(
                "offset {} out of range for group '{}', label '{}'",
                offset, group_name, label
            )
        })?;

        if offset > total {
            return Err(format!(
                "offset {} out of range (len={}) for group '{}', label '{}'",
                offset, total, group_name, label
            ));
        }
        if size > total - offset {
            return Err(format!(
                "requested size {} exceeds buffer at offset {} (len={}) for group '{}', label '{}'",
                size, offset, total, group_name, label
            ));
        }

        let ptr = unsafe { shm_slice.as_mut_ptr().add(offset) };
        Ok(unsafe { std::slice::from_raw_parts_mut(ptr, size) })
    }

    fn memoryview_from_slice(py: Python, slice_ptr: *mut [u8]) -> PyResult<Py<PyAny>> {
        let ptr = slice_ptr as *mut u8 as *mut c_char;
        let len = unsafe { (&*slice_ptr).len() } as isize;
        let mv = unsafe { ffi::PyMemoryView_FromMemory(ptr, len, ffi::PyBUF_WRITE) };
        if mv.is_null() {
            return Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                "failed to create memoryview from slice",
            ));
        }
        Ok(unsafe { Py::from_owned_ptr(py, mv) })
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
}
