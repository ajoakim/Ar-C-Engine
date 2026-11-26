//! GPU upload of imported models and a World-backed scene builder.

use engine_sdk::{
    EntityId, FrameId, Material, MaterialId, MeshHandle, RenderItem, Resources, SceneState,
    SdkError, TextureId, Transform, World,
};

use crate::import::{ImportedMaterial, ImportedMesh, ImportedModel, ImportedNode, ImportedTexture};
use crate::loader::{ModelAssets, RegisteredModel};

/// GPU-side result of [`upload_model`]: one `(mesh, material)` pair per imported mesh.
#[derive(Debug, Clone, PartialEq)]
pub struct UploadedModel {
    /// Index-aligned with the source model's meshes.
    pub meshes: Vec<(MeshHandle, MaterialId)>,
}

fn upload_parts(
    resources: &mut Resources<'_>,
    scene: &mut SceneState,
    meshes: &[&ImportedMesh],
    materials: &[&ImportedMaterial],
    textures: &[&ImportedTexture],
) -> Result<UploadedModel, SdkError> {
    let mut texture_ids: Vec<TextureId> = Vec::with_capacity(textures.len());
    for t in textures {
        texture_ids.push(resources.create_texture("gltf-texture", t.width, t.height, &t.rgba8)?);
    }
    let mut material_ids: Vec<MaterialId> = Vec::with_capacity(materials.len());
    for m in materials {
        let base_color_texture = match m.texture {
            Some(i) => Some(texture_ids.get(i).copied().ok_or_else(|| {
                SdkError::App(format!("material references missing texture {i}"))
            })?),
            None => None,
        };
        material_ids.push(scene.add_material(Material {
            base_color: m.base_color,
            base_color_texture,
        })?);
    }
    let mut out = Vec::with_capacity(meshes.len());
    for m in meshes {
        let material = match m.material {
            Some(i) => material_ids
                .get(i)
                .copied()
                .ok_or_else(|| SdkError::App(format!("mesh references missing material {i}")))?,
            None => MaterialId::DEFAULT,
        };
        let mesh = resources.create_lit_mesh("gltf-mesh", &m.vertices, &m.indices)?;
        out.push((mesh, material));
    }
    Ok(UploadedModel { meshes: out })
}

/// Upload textures, materials and meshes of `model` through SDK resources.
pub fn upload_model(
    resources: &mut Resources<'_>,
    scene: &mut SceneState,
    model: &ImportedModel,
) -> Result<UploadedModel, SdkError> {
    let meshes: Vec<&ImportedMesh> = model.meshes.iter().collect();
    let materials: Vec<&ImportedMaterial> = model.materials.iter().collect();
    let textures: Vec<&ImportedTexture> = model.textures.iter().collect();
    upload_parts(resources, scene, &meshes, &materials, &textures)
}

/// Upload a model previously registered in [`ModelAssets`], resolving its handles.
///
/// Stale or value-less handles produce `SdkError::App`.
pub fn upload_registered(
    resources: &mut Resources<'_>,
    scene: &mut SceneState,
    assets: &ModelAssets,
    model: &RegisteredModel,
) -> Result<UploadedModel, SdkError> {
    let missing = |kind: &str| SdkError::App(format!("registered {kind} asset is missing"));
    let meshes = model
        .meshes
        .iter()
        .map(|h| assets.meshes.get(*h).ok_or_else(|| missing("mesh")))
        .collect::<Result<Vec<_>, _>>()?;
    let materials = model
        .materials
        .iter()
        .map(|h| assets.materials.get(*h).ok_or_else(|| missing("material")))
        .collect::<Result<Vec<_>, _>>()?;
    let textures = model
        .textures
        .iter()
        .map(|h| assets.textures.get(*h).ok_or_else(|| missing("texture")))
        .collect::<Result<Vec<_>, _>>()?;
    upload_parts(resources, scene, &meshes, &materials, &textures)
}

/// Owns an engine [`World`] and the drawables placed in it.
///
/// [`SceneBuilder::flush`] replaces the whole `items` list of the scene: the builder
/// owns the scene's drawables.
pub struct SceneBuilder {
    world: World,
    frame: FrameId,
    drawables: Vec<(EntityId, MeshHandle, MaterialId)>,
}

impl SceneBuilder {
    /// Create an empty builder spawning root entities in `frame`.
    pub fn new(frame: FrameId) -> Self {
        Self {
            world: World::new(),
            frame,
            drawables: Vec::new(),
        }
    }

    /// Underlying world.
    pub fn world(&self) -> &World {
        &self.world
    }

    /// Spawn one drawable mesh as a root entity.
    pub fn spawn_mesh(
        &mut self,
        mesh: MeshHandle,
        material: MaterialId,
        local: Transform,
    ) -> Result<EntityId, SdkError> {
        let id = self.world.spawn(self.frame, local)?;
        self.drawables.push((id, mesh, material));
        Ok(id)
    }

    /// Spawn a whole model: a root entity plus its node hierarchy.
    pub fn spawn_model(
        &mut self,
        model: &ImportedModel,
        uploaded: &UploadedModel,
        root: Transform,
    ) -> Result<EntityId, SdkError> {
        self.spawn_nodes(&model.nodes, uploaded, root)
    }

    /// Spawn a node hierarchy (as held by [`ImportedModel`] or [`RegisteredModel`]).
    pub fn spawn_nodes(
        &mut self,
        nodes: &[ImportedNode],
        uploaded: &UploadedModel,
        root: Transform,
    ) -> Result<EntityId, SdkError> {
        let root_entity = self.world.spawn(self.frame, root)?;
        let mut entities: Vec<EntityId> = Vec::with_capacity(nodes.len());
        let mut pending: Vec<(EntityId, MeshHandle, MaterialId)> = Vec::new();
        for (index, node) in nodes.iter().enumerate() {
            let parent = match node.parent {
                None => root_entity,
                Some(p) if p < index => *entities
                    .get(p)
                    .ok_or_else(|| SdkError::App("node parent order violated".into()))?,
                Some(_) => return Err(SdkError::App("node parent order violated".into())),
            };
            let entity = self.world.spawn_child(parent, node.local)?;
            entities.push(entity);
            for mesh_index in &node.meshes {
                let (mesh, material) =
                    uploaded.meshes.get(*mesh_index).copied().ok_or_else(|| {
                        SdkError::App(format!("node references missing mesh {mesh_index}"))
                    })?;
                pending.push((entity, mesh, material));
            }
        }
        self.drawables.extend(pending);
        Ok(root_entity)
    }

    /// Change an entity's local transform.
    pub fn set_local(&mut self, id: EntityId, local: Transform) -> Result<(), SdkError> {
        self.world.set_local(id, local)?;
        Ok(())
    }

    /// Update world transforms and rewrite `items` (replacing its contents).
    pub fn flush(&mut self, items: &mut Vec<RenderItem>) -> Result<(), SdkError> {
        self.world.update_transforms();
        items.clear();
        for (entity, mesh, material) in &self.drawables {
            let wt = self.world.world_transform(*entity)?;
            let mut item = RenderItem::new(*mesh, wt.position).with_material(*material);
            item.rotation = wt.rotation;
            item.scale = wt.scale;
            items.push(item);
        }
        Ok(())
    }

    /// Number of drawables.
    pub fn drawable_count(&self) -> usize {
        self.drawables.len()
    }
}
