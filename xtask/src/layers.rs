//! Single source of truth for the layer table.
//!
//! Keep in sync with `docs/architecture/dependency-direction.md`; update both together.

/// A workspace crate and its tier.
pub struct Layer {
    pub name: &'static str,
    pub tier: u8,
}

pub const LAYERS: &[Layer] = &[
    Layer {
        name: "engine-math",
        tier: 0,
    },
    Layer {
        name: "engine-diagnostics",
        tier: 0,
    },
    Layer {
        name: "engine-core",
        tier: 1,
    },
    Layer {
        name: "engine-jobs",
        tier: 1,
    },
    Layer {
        name: "engine-procedural",
        tier: 1,
    },
    Layer {
        name: "engine-platform",
        tier: 2,
    },
    Layer {
        name: "engine-assets",
        tier: 2,
    },
    Layer {
        name: "engine-world",
        tier: 2,
    },
    Layer {
        name: "engine-spatial",
        tier: 3,
    },
    Layer {
        name: "engine-render",
        tier: 3,
    },
    Layer {
        name: "engine-streaming",
        tier: 3,
    },
    Layer {
        name: "engine-render-wgpu",
        tier: 4,
    },
    Layer {
        name: "debug-ui",
        tier: 4,
    },
    Layer {
        name: "engine-sdk",
        tier: 5,
    },
    Layer {
        name: "conventional-3d",
        tier: 6,
    },
    Layer {
        name: "planetary-core",
        tier: 6,
    },
    Layer {
        name: "planetary-terrain",
        tier: 7,
    },
    Layer {
        name: "planetary-app",
        tier: 8,
    },
];

/// Tooling packages, exempt from tiers and R5/R6.
pub const TOOLING: &[&str] = &["xtask"];
pub const WGPU_ALLOWED: &[&str] = &["engine-render-wgpu"];
pub const WINIT_ALLOWED: &[&str] = &["engine-platform"];

pub fn tier_of(name: &str) -> Option<u8> {
    LAYERS.iter().find(|l| l.name == name).map(|l| l.tier)
}
