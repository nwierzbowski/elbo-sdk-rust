mod com_types;
mod engine_api;
mod engine_client; // This line remains unchanged
extern crate iceoryx2_loggers;

use pyo3::prelude::*;

#[pymodule(name = "_elbo_sdk_rust")]
mod elbo_sdk_rust {
    use pyo3::prelude::*;

    use iceoryx2_bb_container::semantic_string::SemanticString;
    use iceoryx2_bb_posix::shared_memory::{
        CreationMode, Permission, SharedMemory, SharedMemoryBuilder,
    };
    use std::path::PathBuf;

    use crate::com_types;
    use crate::engine_api;
    use iceoryx2_bb_system_types::file_name::FileName;
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
    struct GroupMemoryViews {
        _shm: SharedMemory, // keep backing alive
        verts: *mut [u8],
        edges: *mut [u8],
        rotations: *mut [u8],
        scales: *mut [u8],
        offsets: *mut [u8],
        vert_counts: *mut [u8],
        edge_counts: *mut [u8],
    }

    #[pyclass(unsendable)]
    struct StandardizeGroupContext {
        pub group_mvs: Vec<GroupMemoryViews>,
        pub group_metadata_vec: Vec<com_types::GroupFull>,
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
            let r = memoryview_from_slice(py, g.rotations)?;
            let s = memoryview_from_slice(py, g.scales)?;
            let o = memoryview_from_slice(py, g.offsets)?;
            let vc = memoryview_from_slice(py, g.vert_counts)?;
            let ec = memoryview_from_slice(py, g.edge_counts)?;

            Ok((v, e, r, s, o, vc, ec))
        }

        fn finalize(&mut self) -> () {
            let response = engine_api::standardize_groups_command(self.group_metadata_vec.clone())
                .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()));
            println!("Standardize Groups Response: {:?}", response);
        }
    }

    /* inlined into `prepare_standardize_groups` below */

    #[pyfunction]
    fn prepare_standardize_groups(
        vert_counts: Vec<u32>,
        edge_counts: Vec<u32>,
        object_counts: Vec<u32>,
        group_names: Vec<String>,
        surface_contexts: Vec<u32>,
    ) -> PyResult<StandardizeGroupContext> {
        let mut group_mvs = Vec::new();
        let mut group_metadata_vec = Vec::new();

        for i in 0..group_names.len() as usize {

            let (group_metadata, group_mv) = prepare_standardize_group(
                &group_names[i],
                vert_counts[i],
                edge_counts[i],
                object_counts[i],
                surface_contexts[i],
            )?;

            group_metadata_vec.push(group_metadata);
            group_mvs.push(group_mv);
        }

        Ok(StandardizeGroupContext {
            group_mvs,
            group_metadata_vec,
        })
    }

    fn prepare_standardize_group(
        group_name: &String,
        total_verts: u32,
        total_edges: u32,
        object_count: u32,
        surface_contexts: u32,
    ) -> PyResult<(com_types::GroupFull, GroupMemoryViews)> {
        let handle_name = new_uid16();

        let (size, group_metadata) = com_types::GroupFull::new(
            total_verts,
            total_edges,
            object_count,
            surface_contexts,
            &group_name,
            &handle_name,
        );


        let shm = create_shm_segment(
            &handle_name,
            size.try_into().expect(&format!(
                "Mesh size {} exceeds system address space (usize)",
                size
            )),
        )
        .map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!(
                "failed to create shared memory for group {}: {}",
                group_name, e
            ))
        })?;

        let base_ptr = shm.base_address().as_ptr() as *mut u8;
        let shm_size = shm.size();
        let shm_slice: &mut [u8] = unsafe { std::slice::from_raw_parts_mut(base_ptr, shm_size) };

        let verts_slice = shm_slice_from_range(
            shm_slice,
            group_metadata.offset_verts,
            (total_verts as usize) * 3 * 4,
            group_name,
            "verts",
        )?;

        let edges_slice = shm_slice_from_range(
            shm_slice,
            group_metadata.offset_edges,
            (total_edges as usize) * 2 * 4,
            group_name,
            "edges",
        )?;

        let rotations_slice = shm_slice_from_range(
            shm_slice,
            group_metadata.offset_rotations,
            (group_metadata.object_count as usize) * 4 * 4,
            group_name,
            "rotations",
        )?;

        let scales_slice = shm_slice_from_range(
            shm_slice,
            group_metadata.offset_scales,
            (group_metadata.object_count as usize) * 3 * 4,
            group_name,
            "scales",
        )?;

        let offsets_slice = shm_slice_from_range(
            shm_slice,
            group_metadata.offset_offsets,
            (group_metadata.object_count as usize) * 3 * 4,
            group_name,
            "offsets",
        )?;

        //+1 for total at the end
        let vert_counts_slice = shm_slice_from_range(
            shm_slice,
            group_metadata.offset_vert_counts,
            ((group_metadata.object_count + 1) as usize) * 4,
            group_name,
            "vert_counts",
        )?;

        //+1 for total at the end
        let edge_counts_slice = shm_slice_from_range(
            shm_slice,
            group_metadata.offset_edge_counts,
            ((group_metadata.object_count + 1) as usize) * 4,
            group_name,
            "edge_counts",
        )?;

        Ok((
            group_metadata,
            GroupMemoryViews {
                _shm: shm,
                verts: verts_slice,
                edges: edges_slice,
                rotations: rotations_slice,
                scales: scales_slice,
                offsets: offsets_slice,
                vert_counts: vert_counts_slice,
                edge_counts: edge_counts_slice,
            },
        ))
    }

    fn shm_slice_from_range(
        shm_slice: &mut [u8],
        offset: u64,
        size: usize,
        group_name: &str,
        label: &str,
    ) -> PyResult<*mut [u8]> {
        let total = shm_slice.len();
        let offset = usize::try_from(offset).map_err(|_| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "offset {} out of range for group '{}'",
                offset, group_name
            ))
        })?;

        if offset > total {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "offset {} out of range (len={}) for group '{}'",
                offset, total, group_name
            )));
        }
        if size > total - offset {
            return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
                "requested size {} exceeds buffer for '{}' at offset {} (len={})",
                size, label, offset, total
            )));
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

    fn create_shm_segment(name: &str, size: usize) -> Result<SharedMemory, String> {
        let file_name = FileName::new(name.as_bytes())
            .map_err(|e| format!("invalid shared memory name '{}': {:?}", name, e))?;

        SharedMemoryBuilder::new(&file_name)
            .is_memory_locked(false)
            .creation_mode(CreationMode::PurgeAndCreate)
            .size(size)
            .permission(Permission::OWNER_ALL | Permission::GROUP_ALL)
            .zero_memory(true)
            .create()
            .map_err(|e| format!("failed to create shared memory '{}': {:?}", name, e))
    }

    // fn memoryview_from_shm(py: Python, shm: &mut SharedMemory) -> PyResult<Py<PyAny>> {
    //     let slice = shm.as_mut_slice();
    //     let ptr = slice.as_mut_ptr() as *mut c_char;
    //     let len = slice.len() as isize;
    //     let mv = unsafe { ffi::PyMemoryView_FromMemory(ptr, len, ffi::PyBUF_WRITE) };
    //     if mv.is_null() {
    //         return Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
    //             "failed to create memoryview for shared memory",
    //         ));
    //     }
    //     Ok(unsafe { Py::<PyAny>::from_owned_ptr(py, mv) })
    // }

    // fn memoryview_from_shm_range(
    //     py: Python,
    //     shm: &mut SharedMemory,
    //     offset: usize,
    //     size: usize,
    // ) -> PyResult<Py<PyAny>> {
    //     let slice = shm.as_mut_slice();
    //     let total = slice.len();

    //     if offset > total {
    //         return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
    //             "offset {} out of range (len={})",
    //             offset, total
    //         )));
    //     }
    //     if size > total - offset {
    //         return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>(format!(
    //             "requested size {} exceeds buffer at offset {} (len={})",
    //             size, offset, total
    //         )));
    //     }

    //     let ptr = unsafe { slice.as_mut_ptr().add(offset) as *mut c_char };
    //     let len = size as isize;
    //     let mv = unsafe { ffi::PyMemoryView_FromMemory(ptr, len, ffi::PyBUF_WRITE) };
    //     if mv.is_null() {
    //         return Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
    //             "failed to create memoryview for shared memory range",
    //         ));
    //     }
    //     Ok(unsafe { Py::<PyAny>::from_owned_ptr(py, mv) })
    // }
}
