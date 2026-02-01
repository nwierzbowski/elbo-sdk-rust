use std::{ptr::NonNull, slice::from_raw_parts_mut};

use pivot_com_types::{MAX_NAME_LEN, asset_meta::AssetMeta, fields::{Edge, Matrix4x4, Uuid, Vert}};

#[derive(Debug)]
pub struct AssetDataSlices {
    pub obj_uuids: *mut [u8],
    pub verts: *mut [u8],
    pub edges: *mut [u8],
    pub transforms: *mut [u8],
    pub vert_counts: *mut [u8],
    pub edge_counts: *mut [u8],
    pub object_names: *mut [u8],
}

impl AssetDataSlices {
    pub fn new(ptr: NonNull<AssetMeta>) -> Result<Self, String> {
        unsafe {
            let meta = ptr.as_ref();

            let ptr = ptr.cast::<u8>();

            let verts_slice = from_raw_parts_mut(
                ptr.as_ptr().add(meta.offset_verts),
                meta.vert_count as usize * size_of::<Vert>(),
            );

            let edges_slice = from_raw_parts_mut(
                ptr.as_ptr().add(meta.offset_edges),
                meta.edge_count as usize * size_of::<Edge>(),
            );

            let transforms_slice = from_raw_parts_mut(ptr.as_ptr().add(meta.offset_transforms), meta.object_count as usize * size_of::<Matrix4x4>());

            // +1 for total at the end
            let vert_counts_slice = from_raw_parts_mut(
                ptr.as_ptr().add(meta.offset_vert_bases),
                (meta.object_count + 1) as usize * size_of::<u32>(),
            );

            // +1 for total at the end
            let edge_counts_slice = from_raw_parts_mut(
                ptr.as_ptr().add(meta.offset_edge_bases),
                (meta.object_count + 1) as usize * size_of::<u32>(),
            );

            let object_names_slice = from_raw_parts_mut(ptr.as_ptr().add(meta.offset_object_names), meta.object_count as usize * MAX_NAME_LEN);

            let obj_uuids_slice = from_raw_parts_mut(ptr.as_ptr().add(meta.offset_uuids), (meta.object_count as usize) * size_of::<Uuid>());

            Ok(AssetDataSlices {
                verts: verts_slice,
                edges: edges_slice,
                transforms: transforms_slice,
                vert_counts: vert_counts_slice,
                edge_counts: edge_counts_slice,
                object_names: object_names_slice,
                obj_uuids: obj_uuids_slice,
            })
        }
    }
}
