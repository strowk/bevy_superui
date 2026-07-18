//! Headless, arena-backed DOM tree for bevy_superui.
//!
//! Knows nothing about Bevy or JavaScript. The structural source of truth that
//! the reconciler diffs against and that the JS layer mutates.

#[cfg(test)]
mod smoke {
    #[test]
    fn crate_builds() {
        assert_eq!(2 + 2, 4);
    }
}
