use crate::error::AppError;

pub(super) struct ContextTaskFailure<R> {
    pub(super) index: usize,
    pub(super) requirement: R,
    pub(super) error: AppError,
    pub(super) elapsed_ms: u64,
    pub(super) available_count: Option<usize>,
}
