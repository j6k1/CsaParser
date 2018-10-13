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
#[derive(Debug)]
pub enum CsaParserError {
	StreamReadError(CsaStreamReadError),
	FormatError(String),
}
impl fmt::Display for CsaParserError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			CsaParserError::StreamReadError(_) => write!(f, "There was an error loading the stream."),
			CsaParserError::FormatError(ref s) => s.fmt(f),
		}
	}
}
impl error::Error for CsaParserError {
	fn description(&self) -> &str {
		match *self {
			CsaParserError::StreamReadError(_) => "There was an error loading the stream.",
			CsaParserError::FormatError(_) => "Invalid format.",
		}
	}

	fn cause(&self) -> Option<&error::Error> {
		match *self {
			CsaParserError::StreamReadError(ref e) => Some(e),
			CsaParserError::FormatError(_) => None,
		}
	}
}
impl From<CsaStreamReadError> for CsaParserError {
	fn from(err: CsaStreamReadError) -> CsaParserError {
		CsaParserError::StreamReadError(err)
	}
}
