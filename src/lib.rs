use pyo3::prelude::*;
use shared_memory::{Shmem, ShmemConf};

struct ShmemWrapper(shared_memory::Shmem);

unsafe impl Send for ShmemWrapper {}
unsafe impl Sync for ShmemWrapper {}

// 1. Define the Python Class
#[pyclass]
struct ElboClient {
    // We keep the shared memory handle alive here so it doesn't close
    // "Box" is like std::unique_ptr in C++ (Heap allocation)
    shmem: Box<ShmemWrapper>,
}

// 2. Define the Methods
#[pymethods]
impl ElboClient {
    // The Constructor (__init__)
    #[new]
    fn new(shm_name: String, size: usize) -> PyResult<Self> {
        // Try to create the shared memory
        let shmem = match ShmemConf::new().size(size).os_id(&shm_name).create() {
            Ok(m) => m,
            Err(shared_memory::ShmemError::LinkExists) => {
                // If it exists, open it instead.
                // We use map_err to convert the specific Shmem error to a generic string for Python
                ShmemConf::new().os_id(&shm_name).open().map_err(|e| {
                    PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string())
                })?
            }
            Err(e) => {
                // Convert Rust error to Python RuntimeError
                return Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(e.to_string()));
            }
        };

        Ok(ElboClient { 
            shmem: Box::new(ShmemWrapper(shmem))
        })
    }

    // A method to write data (Simulating your live-link)
    // Takes a Python list of floats
    fn send_data(&mut self, data: Vec<f32>) -> PyResult<usize> {
        let ptr = self.shmem.0.as_ptr();
        let len = data.len();
        
        // UNSAFE BLOCK: Required for raw pointer manipulation
        unsafe {
            // Cast the raw byte pointer to a float pointer
            let float_ptr = ptr as *mut f32;
            
            // Check bounds (Rust safety!)
            // shmem.len() is in bytes, so we divide by 4 (sizeof f32)
            if len * 4 > self.shmem.0.len() {
                return Err(PyErr::new::<pyo3::exceptions::PyValueError, _>("Data too large for SHM"));
            }

            // Copy data from the Vec to the shared memory
            std::ptr::copy_nonoverlapping(data.as_ptr(), float_ptr, len);
        }

        Ok(len)
    }
    
    // A simple getter to prove it works
    fn read_first_float(&self) -> PyResult<f32> {
        let ptr = self.shmem.0.as_ptr();
        unsafe {
            let float_ptr = ptr as *const f32;
            Ok(*float_ptr)
        }
    }
}

// 3. The Module Entry Point
// UPDATED for PyO3 0.21+ (Bound API)
#[pymodule]
fn elbo_sdk_rust(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Add the class to the module
    m.add_class::<ElboClient>()?;
    Ok(())
}