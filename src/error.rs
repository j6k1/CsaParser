use std::io;
use std::fmt;
use std::error;
use std::num::ParseIntError;

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

	fn source(&self) -> Option<&(dyn error::Error + 'static)> {
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
	ParseIntError(ParseIntError),
	InvalidStateError(String),
}
impl fmt::Display for CsaParserError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			CsaParserError::StreamReadError(_) => write!(f, "There was an error loading the stream."),
			CsaParserError::FormatError(ref s) => s.fmt(f),
			CsaParserError::ParseIntError(ref e) => e.fmt(f),
			CsaParserError::InvalidStateError (ref s) => s.fmt(f),
		}
	}
}
impl error::Error for CsaParserError {
	fn description(&self) -> &str {
		match *self {
			CsaParserError::StreamReadError(_) => "There was an error loading the stream.",
			CsaParserError::FormatError(_) => "Invalid format.",
			CsaParserError::ParseIntError(ref e) => e.description(),
			CsaParserError::InvalidStateError(_) => "Invalid read state.",
		}
	}

	fn source(&self) -> Option<&(dyn error::Error + 'static)> {
		match *self {
			CsaParserError::StreamReadError(ref e) => Some(e),
			CsaParserError::FormatError(_) => None,
			CsaParserError::ParseIntError(ref e) => Some(e),
			CsaParserError::InvalidStateError(_) => None,
		}
	}
}
impl From<CsaStreamReadError> for CsaParserError {
	fn from(err: CsaStreamReadError) -> CsaParserError {
		CsaParserError::StreamReadError(err)
	}
}
impl From<ParseIntError> for CsaParserError {
	fn from(err: ParseIntError) -> CsaParserError {
		CsaParserError::ParseIntError(err)
	}
}
impl From<CsaStateError> for CsaParserError {
	fn from(err: CsaStateError) -> CsaParserError {
		match err {
			CsaStateError::InvalidStateError(s) => CsaParserError::InvalidStateError(s)
		}
	}
}
#[derive(Debug)]
pub enum CsaStateError {
	InvalidStateError(String),
}
impl fmt::Display for CsaStateError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match *self {
			CsaStateError::InvalidStateError (ref s) => s.fmt(f),
		}
	}
}
impl error::Error for CsaStateError {
	fn description(&self) -> &str {
		match *self {
			CsaStateError::InvalidStateError(_) => "Invalid read state.",
		}
	}

	fn source(&self) -> Option<&(dyn error::Error + 'static)> {
		match *self {
			CsaStateError::InvalidStateError(_) => None,
		}
	}
}
