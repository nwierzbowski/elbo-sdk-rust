use iceoryx2_bb_posix::shared_memory::SharedMemory;
use pivot_com_types::{MAX_NAME_LEN, com_types};

#[derive(Debug)]
    pub struct AssetDataSlices {
        // _shm: SharedMemory, // keep backing alive
        pub uuids: *mut [u8],
        pub verts: *mut [u8],
        pub edges: *mut [u8],
        pub transforms: *mut [u8],
        pub vert_counts: *mut [u8],
        pub edge_counts: *mut [u8],
        pub object_names: *mut [u8],
    }

    impl AssetDataSlices {
        pub fn new(
            shm: SharedMemory,
            group_metadata: &com_types::AssetMeta,
        ) -> Result<Self, String> {
            let base_ptr = shm.base_address().as_ptr() as *mut u8;
            let shm_size = shm.size();
            std::mem::forget(shm);
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

            // +1 for total at the end
            let vert_counts_slice = shm_slice_from_range(
                shm_slice,
                group_metadata.offset_vert_bases,
                ((group_metadata.object_count + 1) as usize) * size_of::<u32>(),
                group_name,
                "vert_counts",
            )?;

            // +1 for total at the end
            let edge_counts_slice = shm_slice_from_range(
                shm_slice,
                group_metadata.offset_edge_bases,
                ((group_metadata.object_count + 1) as usize) * size_of::<u32>(),
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
                // _shm: shm,
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