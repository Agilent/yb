use itertools::Itertools;
use serde::Serialize;

#[derive(Debug, PartialEq, Eq, Clone, Copy, PartialOrd, Ord, Serialize)]
pub enum UpstreamComparison {
    UpToDate,
    Behind(usize),
    Ahead(usize),
    Diverged { ahead: usize, behind: usize },
}

impl UpstreamComparison {
    pub fn is_diverged(&self) -> bool {
        matches!(self, UpstreamComparison::Diverged { .. })
    }
}

// It just so happens we can use the ordering of the `UpstreamComparison` enum to determine the
// optimal branch.
pub fn determine_optimal_checkout_branch(
    candidates: &Vec<LocalTrackingBranchWithUpstreamComparison>,
) -> Option<&LocalTrackingBranchWithUpstreamComparison> {
    candidates
        .iter()
        .sorted_by(|a, b| a.upstream_comparison.cmp(&b.upstream_comparison))
        .next()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn upstream_comparison_orderings() {
        assert_eq!(UpstreamComparison::UpToDate, UpstreamComparison::UpToDate);
        assert!(UpstreamComparison::UpToDate < UpstreamComparison::Ahead(10));
        assert!(UpstreamComparison::UpToDate < UpstreamComparison::Behind(10));
        assert!(
            UpstreamComparison::UpToDate
                < UpstreamComparison::Diverged {
                    ahead: 1,
                    behind: 2
                }
        );
        assert!(UpstreamComparison::Behind(1) < UpstreamComparison::Ahead(1));
    }
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct BranchStatus {
    /// Name of the branch
    pub local_branch_name: String,
    pub upstream_branch_status: Option<UpstreamBranchStatus>,
}

impl BranchStatus {
    pub fn is_diverged(&self) -> bool {
        self.upstream_branch_status
            .as_ref()
            .map_or(false, |s| s.upstream_comparison.is_diverged())
    }
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct UpstreamBranchStatus {
    pub remote_tracking_branch: RemoteTrackingBranch,
    /// Status of the branch relative to the upstream
    pub upstream_comparison: UpstreamComparison,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct LocalTrackingBranchWithUpstreamComparison {
    pub local_tracking_branch: LocalTrackingBranch,
    pub upstream_comparison: UpstreamComparison,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
pub struct LocalTrackingBranch {
    pub branch_name: String,
    pub remote_tracking_branch: RemoteTrackingBranch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RemoteTrackingBranch {
    pub remote_name: String,
    pub branch_name: String,
}

impl RemoteTrackingBranch {
    // TODO better name
    pub fn to_string(&self) -> String {
        format!("{}/{}", self.remote_name, self.branch_name)
    }
}
