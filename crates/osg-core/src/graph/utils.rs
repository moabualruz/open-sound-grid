// Utility functions for volume and boolean aggregation.

/// Cubic-root weighted average of volume values (perceptual curve).
pub fn average_volumes<'a>(volumes: impl IntoIterator<Item = &'a f32>) -> f32 {
    let mut count: usize = 0;
    let mut total = 0.0;
    for volume in volumes {
        count += 1;
        total += volume.powf(1.0 / 3.0);
    }
    (total / count.max(1) as f32).powf(3.0)
}

/// True when not all channel volumes are the same.
pub fn volumes_mixed<'a>(volumes: impl IntoIterator<Item = &'a f32>) -> bool {
    let mut iterator = volumes.into_iter();
    let Some(first) = iterator.next() else {
        return false;
    };
    // NOTE: Original Sonusmix logic returns `all(|x| x == first)` — which is
    // true when volumes are NOT mixed. We preserve the original behavior here
    // (the caller in diff_properties assigns the return value to `volume_mixed`
    // just like Sonusmix does).
    iterator.all(|x| x == first)
}

/// `Some(val)` when all booleans agree, `None` when they differ.
pub fn aggregate_bools<'a>(bools: impl IntoIterator<Item = &'a bool>) -> Option<bool> {
    let mut iter = bools.into_iter();
    let first = iter.next()?;
    iter.all(|b| b == first).then_some(*first)
}
