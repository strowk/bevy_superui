//! Allocation-churn measurement (dhat). `backend` is a plain String because the
//! three examples have different backend enums.

#[cfg(feature = "dhat-prof")]
use bevy::prelude::App;

#[derive(Clone, Debug)]
pub struct AllocReport {
    pub backend: String,
    pub frames: usize,
    pub bytes_per_frame: f64,
    pub blocks_per_frame: f64,
}

#[cfg(feature = "dhat-prof")]
pub fn run_alloc_with(
    backend: String,
    build: impl FnOnce() -> App,
    frames: usize,
    warmup: usize,
) -> AllocReport {
    let mut app = build();
    for _ in 0..warmup {
        app.update();
    }
    let before = dhat::HeapStats::get();
    for _ in 0..frames {
        app.update();
    }
    let after = dhat::HeapStats::get();
    let dbytes = after.total_bytes.saturating_sub(before.total_bytes) as f64;
    let dblocks = after.total_blocks.saturating_sub(before.total_blocks) as f64;
    AllocReport {
        backend,
        frames,
        bytes_per_frame: dbytes / frames as f64,
        blocks_per_frame: dblocks / frames as f64,
    }
}

pub fn alloc_table(r: &AllocReport) -> String {
    format!(
        "alloc churn: backend={} frames={} | {:.1} bytes/frame | {:.1} allocs/frame\n",
        r.backend, r.frames, r.bytes_per_frame, r.blocks_per_frame,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alloc_table_reports_per_frame_figures() {
        let r = AllocReport {
            backend: "supersolid".to_string(),
            frames: 100,
            bytes_per_frame: 2048.0,
            blocks_per_frame: 12.0,
        };
        let t = alloc_table(&r);
        assert!(t.contains("backend=supersolid"), "{t}");
        assert!(t.contains("2048.0 bytes/frame"), "{t}");
        assert!(t.contains("12.0 allocs/frame"), "{t}");
    }
}
