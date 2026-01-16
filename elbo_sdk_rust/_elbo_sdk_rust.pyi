from typing import List, Dict, Tuple, Optional, Any


def start_engine(path: str) -> None: ...

def stop_engine() -> None: ...


def standardize_groups_command(
    verts_shm_name: str,
    edges_shm_name: str,
    rotations_shm_name: str,
    scales_shm_name: str,
    offsets_shm_name: str,
    vert_counts: List[int],
    edge_counts: List[int],
    object_counts: List[int],
    group_names: List[str],
    surface_contexts: List[str],
) -> str: ...


def standardize_synced_groups_command(
    group_names: List[str],
    surface_contexts: List[str],
) -> str: ...


def set_surface_types_command(group_surface_map: Dict[str, int]) -> str: ...


def drop_groups_command(group_names: List[str]) -> str: ...


def get_surface_types_command() -> str: ...


def get_license_command() -> str: ...


def get_platform_id() -> str: ...


def get_engine_binary_path() -> Optional[str]: ...


class StandardizeObjectContext:
    def buffers(self) -> Tuple[memoryview, memoryview, memoryview, memoryview, memoryview]: ...
    def finalize(self) -> str: ...


class StandardizeGroupContext:
    def buffers(self) -> Tuple[memoryview, memoryview, memoryview, memoryview, memoryview]: ...
    def finalize(self) -> str: ...


def prepare_standardize_objects(
    total_verts: int,
    total_edges: int,
    total_objects: int,
    vert_counts: List[int],
    edge_counts: List[int],
    object_names: List[str],
    surface_contexts: List[str],
) -> StandardizeObjectContext: ...


def prepare_standardize_groups(
    total_verts: int,
    total_edges: int,
    total_objects: int,
    vert_counts: List[int],
    edge_counts: List[int],
    object_counts: List[int],
    group_names: List[str],
    surface_contexts: List[str],
) -> StandardizeGroupContext: ...
