//! Asynchronous model loading on the engine `JobSystem` plus `AssetRegistry` registration.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::mpsc;

use engine_sdk::{AssetId, AssetRegistry, Handle, JobHandle, JobStatus, JobSystem, Priority};

use crate::error::ImportError;
use crate::import::{
    ImportLimits, ImportedMaterial, ImportedMesh, ImportedModel, ImportedNode, ImportedTexture,
    import_gltf_path_with,
};

/// Identifies one submitted import.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LoadTicket(u64);

/// Result of one import job.
#[derive(Debug)]
pub struct LoadResult {
    /// Ticket returned by [`ModelLoader::load`].
    pub ticket: LoadTicket,
    /// Path that was imported.
    pub path: PathBuf,
    /// Import outcome.
    pub result: Result<ImportedModel, ImportError>,
}

type Message = (LoadTicket, PathBuf, Result<ImportedModel, ImportError>);

/// Runs glTF imports on worker threads and delivers results over a channel.
pub struct ModelLoader {
    jobs: JobSystem,
    tx: mpsc::Sender<Message>,
    rx: mpsc::Receiver<Message>,
    next: u64,
    pending: BTreeMap<LoadTicket, (PathBuf, JobHandle)>,
    ready: Vec<LoadResult>,
}

fn status_error(status: Option<JobStatus>) -> ImportError {
    match status {
        Some(JobStatus::Cancelled) => ImportError::JobFailed("import job cancelled".into()),
        Some(JobStatus::Panicked) => ImportError::JobFailed("import job panicked".into()),
        _ => ImportError::JobFailed("import job produced no result".into()),
    }
}

impl ModelLoader {
    /// Create a loader with `workers` threads (clamped to at least 1).
    pub fn new(workers: usize) -> Result<Self, ImportError> {
        let jobs = JobSystem::builder()
            .workers(workers.max(1))
            .thread_name("model-import")
            .build()
            .map_err(|e| ImportError::JobFailed(e.to_string()))?;
        let (tx, rx) = mpsc::channel();
        Ok(Self {
            jobs,
            tx,
            rx,
            next: 0,
            pending: BTreeMap::new(),
            ready: Vec::new(),
        })
    }

    /// Submit an import job.
    pub fn load(&mut self, path: impl Into<PathBuf>, limits: ImportLimits) -> LoadTicket {
        let ticket = LoadTicket(self.next);
        self.next += 1;
        let p: PathBuf = path.into();
        let tx = self.tx.clone();
        let job_path = p.clone();
        let handle = self.jobs.submit(Priority::Normal, move |tok| {
            if tok.is_cancelled() {
                return;
            }
            let r = import_gltf_path_with(&job_path, &limits);
            let _ = tx.send((ticket, job_path, r));
        });
        self.pending.insert(ticket, (p, handle));
        ticket
    }

    /// Move every message currently in the channel into `ready` / the returned map.
    fn drain(&mut self, want: Option<LoadTicket>) -> Option<Result<ImportedModel, ImportError>> {
        let mut wanted = None;
        while let Ok((ticket, path, result)) = self.rx.try_recv() {
            if Some(ticket) == want {
                wanted = Some(result);
            } else {
                self.pending.remove(&ticket);
                self.ready.push(LoadResult {
                    ticket,
                    path,
                    result,
                });
            }
        }
        wanted
    }

    /// Non-blocking: collect finished results.
    pub fn poll(&mut self) -> Vec<LoadResult> {
        let snapshot: Vec<(LoadTicket, Option<JobStatus>)> = self
            .pending
            .iter()
            .map(|(t, (_, h))| (*t, h.try_status()))
            .collect();
        let _ = self.drain(None);
        for (ticket, status) in snapshot {
            if matches!(status, Some(JobStatus::Panicked | JobStatus::Cancelled))
                && let Some((path, _)) = self.pending.remove(&ticket)
            {
                self.ready.push(LoadResult {
                    ticket,
                    path,
                    result: Err(status_error(status)),
                });
            }
        }
        let mut out = std::mem::take(&mut self.ready);
        out.sort_by_key(|r| r.ticket);
        out
    }

    /// Blocking: wait for one ticket.
    pub fn wait(&mut self, ticket: LoadTicket) -> Result<ImportedModel, ImportError> {
        let Some((_, handle)) = self.pending.remove(&ticket) else {
            return Err(ImportError::JobFailed("unknown load ticket".into()));
        };
        let status = handle.wait();
        match self.drain(Some(ticket)) {
            Some(r) => r,
            None => Err(status_error(Some(status))),
        }
    }

    /// Blocking: wait for every pending job.
    pub fn wait_all(&mut self) -> Vec<LoadResult> {
        let pending = std::mem::take(&mut self.pending);
        let mut statuses = Vec::new();
        for (ticket, (path, handle)) in pending {
            statuses.push((ticket, path, handle.wait()));
        }
        let _ = self.drain(None);
        for (ticket, path, status) in statuses {
            if !self.ready.iter().any(|r| r.ticket == ticket) {
                self.ready.push(LoadResult {
                    ticket,
                    path,
                    result: Err(status_error(Some(status))),
                });
            }
        }
        let mut out = std::mem::take(&mut self.ready);
        out.sort_by_key(|r| r.ticket);
        out
    }

    /// Number of jobs not yet collected.
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }
}

/// Handles of a registered model.
#[derive(Debug, Clone)]
pub struct RegisteredModel {
    /// Mesh handles.
    pub meshes: Vec<Handle<ImportedMesh>>,
    /// Material handles.
    pub materials: Vec<Handle<ImportedMaterial>>,
    /// Texture handles.
    pub textures: Vec<Handle<ImportedTexture>>,
    /// Scene nodes.
    pub nodes: Vec<ImportedNode>,
}

/// Typed registries for imported CPU assets.
#[derive(Default)]
pub struct ModelAssets {
    /// Meshes.
    pub meshes: AssetRegistry<ImportedMesh>,
    /// Materials.
    pub materials: AssetRegistry<ImportedMaterial>,
    /// Textures.
    pub textures: AssetRegistry<ImportedTexture>,
}

fn asset_id(key: &str, kind: &str, i: usize) -> Result<AssetId, ImportError> {
    AssetId::from_key(&format!("{key}#{kind}/{i}")).map_err(|e| ImportError::Asset(e.to_string()))
}

impl ModelAssets {
    /// Create empty registries.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register all parts of `model` under ids derived from `key`.
    pub fn register(
        &mut self,
        key: &str,
        model: ImportedModel,
    ) -> Result<RegisteredModel, ImportError> {
        // Compute all ids first so a bad key leaves the registries untouched.
        let mesh_ids = (0..model.meshes.len())
            .map(|i| asset_id(key, "mesh", i))
            .collect::<Result<Vec<_>, _>>()?;
        let mat_ids = (0..model.materials.len())
            .map(|i| asset_id(key, "material", i))
            .collect::<Result<Vec<_>, _>>()?;
        let tex_ids = (0..model.textures.len())
            .map(|i| asset_id(key, "texture", i))
            .collect::<Result<Vec<_>, _>>()?;
        let meshes = mesh_ids
            .into_iter()
            .zip(model.meshes)
            .map(|(id, m)| self.meshes.insert(id, m))
            .collect();
        let materials = mat_ids
            .into_iter()
            .zip(model.materials)
            .map(|(id, m)| self.materials.insert(id, m))
            .collect();
        let textures = tex_ids
            .into_iter()
            .zip(model.textures)
            .map(|(id, t)| self.textures.insert(id, t))
            .collect();
        Ok(RegisteredModel {
            meshes,
            materials,
            textures,
            nodes: model.nodes,
        })
    }
}
