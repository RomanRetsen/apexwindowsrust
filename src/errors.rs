use thiserror::Error;

#[derive(Error, Debug)]
pub enum DLLError {
    #[error("Windows error code: {0}")]
    WindowsError(u32),
    #[error("Invalid path: {0}")]
    InvalidPath(String),
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    #[error("Invalid DLL")]
    InvalidDLL,
    #[error("DLL Load failed: {0}")]
    LoadError(&'static str),
    #[error("Allocation failure")]
    AllocationFailure,
    #[error("Not a PE file")]
    NotAPE,
    #[error("Not a DLL file")]
    NotADLL,
    #[error("DLL Main returned a failure")]
    DLLMainFailed,
}
