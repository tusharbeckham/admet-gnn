//! Batch job lifecycle and progress.
//!
//! Manual chapter 20.3, and FR-15. The design constraint is that a 10,000-row
//! screen takes minutes, so the job must be observable while it runs and
//! resumable if the process dies -- neither of which is true of a job whose only
//! state is a running future.

use sqlx::PgPool;
use uuid::Uuid;

use crate::model::{Batch, BatchStatus};
use crate::{DbError, Result};

/// Queries over the `batches` table.
#[derive(Debug, Clone)]
pub struct BatchRepo {
    pool: PgPool,
}

impl BatchRepo {
    /// Wrap a pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// The underlying pool, for callers running these queries in their own
    /// transaction.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Create a queued job. `total_rows` is counted from the uploaded CSV before
    /// any work starts, so progress can be reported as a fraction from the first
    /// checkpoint rather than as a bare count that means nothing on its own.
    ///
    /// # Errors
    /// [`DbError::NotImplemented`] until Increment 4.
    pub async fn create(&self, project_id: Uuid, name: &str, total_rows: u32) -> Result<Batch> {
        let _ = (project_id, name, total_rows);
        Err(DbError::NotImplemented("BatchRepo::create"))
    }

    /// Record progress at a checkpoint.
    ///
    /// Called once per [`super::CHECKPOINT_INTERVAL`] rows, not once per row.
    /// The write is a single `UPDATE ... SET completed_rows = $2, failed_rows =
    /// $3` on one indexed row, so it costs nothing at that frequency and
    /// everything at per-row frequency.
    ///
    /// # Errors
    /// [`DbError::NotImplemented`] until Increment 4.
    pub async fn checkpoint(&self, id: Uuid, completed: u32, failed: u32) -> Result<()> {
        let _ = (id, completed, failed);
        Err(DbError::NotImplemented("BatchRepo::checkpoint"))
    }

    /// Move the job to a terminal state and stamp `finished_at`.
    ///
    /// Takes [`BatchStatus`] rather than a string so an invalid transition is a
    /// type error at the call site. The database keeps a `CHECK` constraint on
    /// the column as well: the type protects this crate's callers, the constraint
    /// protects the table from anything that reaches it another way.
    ///
    /// # Errors
    /// [`DbError::NotImplemented`] until Increment 4.
    pub async fn finish(&self, id: Uuid, status: BatchStatus) -> Result<()> {
        let _ = (id, status);
        Err(DbError::NotImplemented("BatchRepo::finish"))
    }

    /// Fetch one job, for the progress-polling endpoint.
    ///
    /// # Errors
    /// [`DbError::NotFound`] if absent; [`DbError::NotImplemented`] until
    /// Increment 4.
    pub async fn find_by_id(&self, id: Uuid) -> Result<Batch> {
        let _ = id;
        Err(DbError::NotImplemented("BatchRepo::find_by_id"))
    }

    /// Jobs left `running` by a process that died.
    ///
    /// Found by `status = 'running'` with a stale `created_at`, and it is the
    /// query that makes checkpointing worth anything: on start-up the service
    /// either resumes these from `completed_rows` or marks them failed, so a
    /// crash leaves no job stuck at 40% forever with a spinner in front of a
    /// user who has no way to tell that nothing is happening.
    ///
    /// # Errors
    /// [`DbError::NotImplemented`] until Increment 4.
    pub async fn find_orphaned(&self, older_than_minutes: i32) -> Result<Vec<Batch>> {
        let _ = older_than_minutes;
        Err(DbError::NotImplemented("BatchRepo::find_orphaned"))
    }

    /// Recent jobs for a project, newest first.
    ///
    /// # Errors
    /// [`DbError::NotImplemented`] until Increment 4.
    pub async fn list_for_project(&self, project_id: Uuid, limit: i64) -> Result<Vec<Batch>> {
        let _ = (project_id, limit);
        Err(DbError::NotImplemented("BatchRepo::list_for_project"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `finish` takes a status, so nothing stops a caller passing a
    /// non-terminal one. The guard belongs in the implementation; this test
    /// records the intent while the body is still a stub, so it is not
    /// rediscovered as a bug in Increment 4.
    #[test]
    fn only_terminal_states_are_valid_arguments_to_finish() {
        let valid: Vec<BatchStatus> = [
            BatchStatus::Queued,
            BatchStatus::Running,
            BatchStatus::Completed,
            BatchStatus::Failed,
            BatchStatus::Cancelled,
        ]
        .into_iter()
        .filter(|s| s.is_terminal())
        .collect();
        assert_eq!(
            valid.len(),
            3,
            "completed, failed, cancelled -- and nothing else"
        );
    }

    #[test]
    fn unimplemented_methods_report_their_own_name() {
        let e = DbError::NotImplemented("BatchRepo::checkpoint");
        assert!(e.to_string().contains("checkpoint"));
    }
}
