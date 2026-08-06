//! Integration tests that require a real ScyllaDB cluster, started via CCM
//! (Cassandra Cluster Manager) through the `scylla-ccm-bridge` crate.
//!
//! These tests are excluded from `make run-test-unit` (which runs no cluster)
//! and are executed as part of the Scylla integration job instead. The
//! exclusion is done by test-path substring `ccm` — keep every test in this
//! module tree under the `ccm::` path.

use scylla_ccm_bridge::CLUSTER_VERSION;

mod tls;

/// Returns whether `version` is a *partial* (not fully-qualified) `scylla-ccm`
/// cluster version, i.e. one that ccm cannot resolve to an install directory
/// without hitting S3.
///
/// See [`warn_if_partial_cluster_version`] for why this matters.
fn is_partial_version(version: &str) -> bool {
    match version.split_once(':') {
        // A release version needs a patch component to name a directory,
        // unless it is a release candidate (e.g. `release:2026.1.0~rc3`).
        Some(("release", rest)) => !rest.contains('~') && rest.split('.').count() < 3,
        // Unstable versions are pinned by build timestamp; `latest` is not.
        Some((_, rest)) => rest == "latest",
        // Without a prefix there is nothing for ccm to resolve against S3, so
        // treat a bare version (e.g. `2026.2`) as fully-qualified.
        None => false,
    }
}

/// Warns (once per test binary) if the configured cluster version is not a
/// fully-qualified one.
///
/// `scylla-ccm` re-resolves the cluster version on *every* `ccm` invocation,
/// ignoring the install directory it already recorded in `cluster.conf`. If
/// the version names an already-downloaded directory (e.g. `release:2026.1.0`),
/// resolving it is a local directory lookup and costs nothing. If it is partial
/// (e.g. `release:2026.1`), ccm has to list an S3 bucket to find the matching
/// patch release, and on top of that sleeps for a random 0-5 seconds. That adds
/// several seconds to *each* `ccm` call, which then dominates the runtime of
/// these tests.
pub(crate) fn warn_if_partial_cluster_version() {
    static ONCE: std::sync::Once = std::sync::Once::new();

    ONCE.call_once(|| {
        let version: &str = &CLUSTER_VERSION;
        if is_partial_version(version) {
            tracing::warn!(
                "SCYLLA_TEST_CLUSTER is set to the partial version {version:?}. \
                 Every `ccm` invocation will hit S3 to resolve it, adding several \
                 seconds each. Use a fully-qualified version (e.g. \
                 \"release:2026.1.0\") to make these tests much faster."
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::is_partial_version;

    #[test]
    fn classifies_cluster_versions() {
        // A release with a patch component names a directory directly.
        assert!(!is_partial_version("release:2026.2.2"));
        // A release candidate is fully-qualified even without a third component.
        assert!(!is_partial_version("release:2026.1.0~rc3"));
        // A release without a patch component must be resolved against S3.
        assert!(is_partial_version("release:2026.2"));
        // `latest` is a moving target and must be resolved against S3.
        assert!(is_partial_version("unstable/master:latest"));
        // A timestamp-pinned unstable version is fully-qualified.
        assert!(!is_partial_version("unstable/master:2026-01-01T00-00-00Z"));
        // A bare version without a prefix is treated as fully-qualified.
        assert!(!is_partial_version("2026.2"));
    }
}
