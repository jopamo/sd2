use crate::model::Pipeline;
use crate::reporter::Report;

/// Enforce policies that affect the pipeline configuration before execution.
/// E.g., validate_only forces dry_run.
pub fn enforce_pre_execution(pipeline: &mut Pipeline) {
    if pipeline.validate_only {
        pipeline.dry_run = true;
    }
}

pub struct PolicyEnforcer<'a> {
    pipeline: &'a Pipeline,
}

impl<'a> PolicyEnforcer<'a> {
    pub fn new(pipeline: &'a Pipeline) -> Self {
        Self { pipeline }
    }

    /// Check if staging is allowed for this pipeline.
    pub fn should_stage(&self) -> bool {
         // validate_only implicitly prevents staging via dry_run (if enforced),
         // but we should be explicit.
         if self.pipeline.validate_only {
             return false;
         }
         
         // In "transaction all" mode, we stage.
         if self.pipeline.transaction == crate::model::Transaction::All {
             return true;
         }

         false
    }
    
    /// Check if writing is allowed for a specific file application.
    /// Returns true if we should proceed with write/stage.
    pub fn can_write(&self, modified: bool) -> bool {
        if !modified {
            return false;
        }
        if self.pipeline.dry_run {
            return false;
        }
        if self.pipeline.no_write {
            return false;
        }
        true
    }

    /// Enforce policies after all inputs have been processed but before commit.
    /// This updates the report with policy violations if any.
    pub fn enforce_post_run(&self, report: &mut Report) {
        if self.pipeline.require_match && report.replacements == 0 {
            report.policy_violation = Some("No matches found (--require-match)".into());
        } else if let Some(expected) = self.pipeline.expect {
            if report.replacements != expected {
                report.policy_violation = Some(format!(
                    "Expected {} replacements, found {} (--expect)",
                    expected, report.replacements
                ));
            }
        } else if self.pipeline.fail_on_change && report.modified > 0 {
            report.policy_violation = Some(format!(
                "Changes detected in {} files (--fail-on-change)",
                report.modified
            ));
        }
    }

    /// Check if the transaction manager should commit.
    pub fn should_commit(&self, report: &Report) -> bool {
        // If validate_only, never commit.
        if self.pipeline.validate_only {
            return false;
        }
        
        // If dry_run, never commit.
        if self.pipeline.dry_run {
            return false;
        }

        // Only commit if the run was successful (exit code 0)
        report.exit_code() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Pipeline;

    fn default_pipeline() -> Pipeline {
        Pipeline::default()
    }

    #[test]
    fn enforce_pre_execution_forces_dry_run_if_validate_only() {
        let mut p = default_pipeline();
        p.validate_only = true;
        p.dry_run = false;

        enforce_pre_execution(&mut p);
        assert!(p.dry_run);
    }

    #[test]
    fn should_stage_returns_true_for_transaction_all() {
        let mut p = default_pipeline();
        p.transaction = crate::model::Transaction::All;
        let enforcer = PolicyEnforcer::new(&p);
        assert!(enforcer.should_stage());
    }

    #[test]
    fn should_stage_returns_false_if_validate_only() {
        let mut p = default_pipeline();
        p.transaction = crate::model::Transaction::All;
        p.validate_only = true;
        let enforcer = PolicyEnforcer::new(&p);
        assert!(!enforcer.should_stage());
    }

    #[test]
    fn can_write_returns_false_if_not_modified() {
        let p = default_pipeline();
        let enforcer = PolicyEnforcer::new(&p);
        assert!(!enforcer.can_write(false));
    }

    #[test]
    fn can_write_returns_false_if_dry_run() {
        let mut p = default_pipeline();
        p.dry_run = true;
        let enforcer = PolicyEnforcer::new(&p);
        assert!(!enforcer.can_write(true));
    }

    #[test]
    fn can_write_returns_false_if_no_write() {
        let mut p = default_pipeline();
        p.no_write = true;
        let enforcer = PolicyEnforcer::new(&p);
        assert!(!enforcer.can_write(true));
    }

    #[test]
    fn can_write_returns_true_if_modified_and_allowed() {
        let p = default_pipeline();
        let enforcer = PolicyEnforcer::new(&p);
        assert!(enforcer.can_write(true));
    }

    #[test]
    fn enforce_post_run_require_match() {
        let mut p = default_pipeline();
        p.require_match = true;
        let enforcer = PolicyEnforcer::new(&p);
        let mut report = Report::new(false, false);
        report.replacements = 0;

        enforcer.enforce_post_run(&mut report);
        assert!(report.policy_violation.is_some());
        assert!(report.policy_violation.unwrap().contains("No matches found"));
    }

    #[test]
    fn enforce_post_run_fail_on_change() {
        let mut p = default_pipeline();
        p.fail_on_change = true;
        let enforcer = PolicyEnforcer::new(&p);
        let mut report = Report::new(false, false);
        report.modified = 1;

        enforcer.enforce_post_run(&mut report);
        assert!(report.policy_violation.is_some());
        assert!(report.policy_violation.unwrap().contains("Changes detected"));
    }

    #[test]
    fn should_commit_returns_false_if_validate_only() {
        let mut p = default_pipeline();
        p.validate_only = true;
        let enforcer = PolicyEnforcer::new(&p);
        let report = Report::new(false, true);
        assert!(!enforcer.should_commit(&report));
    }

    #[test]
    fn should_commit_returns_false_if_dry_run() {
        let mut p = default_pipeline();
        p.dry_run = true;
        let enforcer = PolicyEnforcer::new(&p);
        let report = Report::new(true, false);
        assert!(!enforcer.should_commit(&report));
    }
}