use std::io;
use std::fmt;
use std::error;

#[derive(Debug)]
pub enum CsaStreamReadError {
	IOError(io::Error),
}
impl fmt::Display for CsaStreamReadError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			CsaStreamReadError::IOError(_) => write!(f, "Error occurred in file I/O."),
		}
	}
}
impl error::Error for CsaStreamReadError {
	fn description(&self) -> &str {
		match *self {
			CsaStreamReadError::IOError(_) => "Error occurred in file I/O.",
		}
	}

	fn cause(&self) -> Option<&error::Error> {
		match *self {
			CsaStreamReadError::IOError(ref e) => Some(e),
		}
	}
}
impl From<io::Error> for CsaStreamReadError {
	fn from(err: io::Error) -> CsaStreamReadError {
		CsaStreamReadError::IOError(err)
	}
}
