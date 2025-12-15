use crate::error::Result;
use crate::write::StagedEntry;

pub struct TransactionManager {
    staged: Vec<StagedEntry>,
}

impl TransactionManager {
    pub fn new() -> Self {
        Self { staged: Vec::new() }
    }

    pub fn stage(&mut self, entry: StagedEntry) {
        self.staged.push(entry);
    }

    pub fn commit(self) -> Result<()> {
        for entry in self.staged {
            entry.commit()?;
        }
        Ok(())
    }

    // Rollback is automatic: StagedEntry holds NamedTempFile.
    // When TransactionManager is dropped (if not committed),
    // the Vec is dropped, NamedTempFiles are dropped,
    // and temp files are deleted by tempfile crate destructor.
}
