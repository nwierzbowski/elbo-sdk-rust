use pivot_com_types::com_types;
use pyo3::{ffi, prelude::*};
use std::os::raw::c_char;

use crate::{asset_data_slices::AssetDataSlices, engine_api};

#[pyclass(unsendable)]
pub struct AssetSyncContext {
    pub asset_slices: Vec<AssetDataSlices>,
    pub shm_offsets: Option<Vec<com_types::ShmOffset>>,
}

#[pymethods]
impl AssetSyncContext {
    pub fn buffers(
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
        let g = self.asset_slices.get(i).ok_or_else(|| {
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

    pub fn finalize(&mut self) -> () {
        let offsets = match std::mem::take(&mut self.shm_offsets) {
            Some(offsets) => offsets,
            None => return,
        };
        let response = engine_api::standardize_groups_command(offsets)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()));
        println!("Standardize Groups Response: {:?}", response);
    }
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
