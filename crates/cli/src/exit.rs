//! Exit-code mapping for the `pdfbro` binary.
//!
//! The numeric codes are wired into the binary so test harnesses
//! (`assert_cmd`) and shell pipelines can branch on them. The mapping is
//! authoritative and matches `docs/specs/20-cli.md` § *Exit codes*.

use engine::EngineError;

/// Sentinel error: maps to [`ExitCode::Usage`].
#[derive(Debug)]
pub(crate) struct UsageError;

impl std::fmt::Display for UsageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("usage error")
    }
}

impl std::error::Error for UsageError {}

/// Sentinel error: maps to [`ExitCode::BatchPartial`].
#[derive(Debug)]
pub(crate) struct BatchPartialFailure {
    /// Number of failed items in the batch run.
    pub(crate) count: usize,
}

impl std::fmt::Display for BatchPartialFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "batch had {} failures", self.count)
    }
}

impl std::error::Error for BatchPartialFailure {}

/// Process exit codes used by the `pdfbro` binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub(crate) enum ExitCode {
    /// `0` — success (unused as an `Err`; kept for symmetry).
    #[allow(dead_code)]
    Success = 0,
    /// `1` — generic / unexpected error (last-resort fallthrough).
    Generic = 1,
    /// `2` — usage / parse error. Delegated to clap when possible.
    Usage = 2,
    /// `3` — engine error (any [`EngineError`] variant other than
    /// [`EngineError::Timeout`] or [`EngineError::Io`]).
    Engine = 3,
    /// `4` — operation timed out.
    Timeout = 4,
    /// `5` — I/O error reading inputs / writing outputs.
    Io = 5,
    /// `6` — `batch --on-error skip` finished with one or more failures.
    BatchPartial = 6,
}

impl ExitCode {
    pub(crate) fn as_i32(self) -> i32 {
        self as i32
    }
}

/// Map an [`EngineError`] to its [`ExitCode`].
pub(crate) fn exit_for_engine(err: &EngineError) -> ExitCode {
    match err {
        EngineError::Timeout(_) => ExitCode::Timeout,
        EngineError::Io(_) => ExitCode::Io,
        _ => ExitCode::Engine,
    }
}

/// Walk an [`anyhow::Error`]'s typed cause chain looking for an
/// [`EngineError`] or known sentinel. Returns the matching exit code,
/// otherwise [`ExitCode::Generic`].
pub(crate) fn exit_for_anyhow(err: &anyhow::Error) -> ExitCode {
    if err.downcast_ref::<UsageError>().is_some() {
        return ExitCode::Usage;
    }
    if err.downcast_ref::<BatchPartialFailure>().is_some() {
        return ExitCode::BatchPartial;
    }
    if let Some(eng) = err.downcast_ref::<EngineError>() {
        return exit_for_engine(eng);
    }
    if err.downcast_ref::<std::io::Error>().is_some() {
        return ExitCode::Io;
    }
    ExitCode::Generic
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn timeout_maps_to_4() {
        let err = EngineError::Timeout(Duration::from_secs(1));
        assert_eq!(exit_for_engine(&err), ExitCode::Timeout);
    }

    #[test]
    fn io_maps_to_5() {
        let err = EngineError::Io(std::io::Error::other("boom"));
        assert_eq!(exit_for_engine(&err), ExitCode::Io);
    }

    #[test]
    fn other_engine_errors_map_to_3() {
        let err = EngineError::InvalidOption("nope".into());
        assert_eq!(exit_for_engine(&err), ExitCode::Engine);
        let err = EngineError::Internal("internal".into());
        assert_eq!(exit_for_engine(&err), ExitCode::Engine);
    }

    #[test]
    fn anyhow_wraps_engine_error_chain() {
        let inner = EngineError::InvalidOption("bad".into());
        let wrapped: anyhow::Error = anyhow::Error::new(inner).context("while doing something");
        assert_eq!(exit_for_anyhow(&wrapped), ExitCode::Engine);
    }

    #[test]
    fn anyhow_wraps_io_error_chain() {
        let inner = std::io::Error::other("disk full");
        let wrapped: anyhow::Error = anyhow::Error::new(inner).context("writing output");
        assert_eq!(exit_for_anyhow(&wrapped), ExitCode::Io);
    }

    #[test]
    fn anyhow_unrelated_error_maps_generic() {
        let wrapped = anyhow::anyhow!("just a string");
        assert_eq!(exit_for_anyhow(&wrapped), ExitCode::Generic);
    }

    #[test]
    fn usage_sentinel_maps_to_2() {
        let wrapped: anyhow::Error = anyhow::Error::new(UsageError).context("nope");
        assert_eq!(exit_for_anyhow(&wrapped), ExitCode::Usage);
    }

    #[test]
    fn batch_partial_sentinel_maps_to_6() {
        let wrapped: anyhow::Error =
            anyhow::Error::new(BatchPartialFailure { count: 3 }).context("batch");
        assert_eq!(exit_for_anyhow(&wrapped), ExitCode::BatchPartial);
    }
}
