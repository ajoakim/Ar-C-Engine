//! Spikes S1-S4 for the half-cell hex topology (HEX-TOPOLOGY.md 3.5). Test-only scratch code;
//! production code is written in 07.1-10/12/14/15 if the user says go.
//!
//! These plans cover S1 (`lattice`: diamond ids, wrap, neighbours, parents/children) and S2
//! (`projection`: exact-op barycentric mapping), S3 (`coverage`: half-cell piece coverage and
//! balance on the sphere) and S4 (`gap`: skirt gap bound at L/L+1 borders). Run with `--nocapture`
//! to see the evidence.

mod coverage;
mod gap;
mod lattice;
mod projection;
