use std::{ptr::NonNull, slice::from_raw_parts_mut};

use pivot_com_types::asset_meta::AssetMeta;

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

            Ok(AssetDataSlices {
                verts: from_raw_parts_mut(ptr.as_ptr().add(meta.offset_verts), meta.verts_byte_size()),
                edges: from_raw_parts_mut(ptr.as_ptr().add(meta.offset_edges), meta.edges_byte_size()),
                transforms: from_raw_parts_mut(
                    ptr.as_ptr().add(meta.offset_transforms),
                    meta.transforms_byte_size(),
                ),
                vert_counts: from_raw_parts_mut(
                    ptr.as_ptr().add(meta.offset_vert_bases),
                    meta.vert_counts_byte_size(),
                ),
                edge_counts: from_raw_parts_mut(
                    ptr.as_ptr().add(meta.offset_edge_bases),
                    meta.edge_counts_byte_size(),
                ),
                object_names: from_raw_parts_mut(
                    ptr.as_ptr().add(meta.offset_object_names),
                    meta.object_names_byte_size(),
                ),
                obj_uuids: from_raw_parts_mut(ptr.as_ptr().add(meta.offset_uuids), meta.uuids_byte_size()),
            })
        }
    }
}
