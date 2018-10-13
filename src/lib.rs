extern crate usiagent;

pub mod error;

use std::io;
use std::io::BufReader;
use std::io::BufRead;
use std::fs::File;
use std::fs::OpenOptions;
use std::collections::HashMap;

use usiagent::error::*;
use usiagent::shogi::*;
use usiagent::TryFrom;

use error::*;

pub trait CsaStream {
	fn next(&mut self) -> Result<Option<String>,CsaStreamReadError>;
	fn read_real_line(&self,l:String) -> Vec<String> {
		if l.starts_with('\'') {
			vec![String::from(&l.as_str()[1..])]
		} else {
			l.split(",").collect::<Vec<&str>>()
									.into_iter()
									.map(|s| s.to_string()).collect::<Vec<String>>()
		}
	}
}
pub struct CsaFileStream {
	reader:BufReader<File>,
	lines:Option<Vec<String>>,
	current_pos:u32,
}
impl CsaFileStream {
	pub fn new(file:&str) -> Result<CsaFileStream,io::Error> {
		let mut reader = BufReader::new(OpenOptions::new().read(true).create(false).open(file)?);
		let mut buf = String::new();
		let n = reader.read_line(&mut buf)?;

		buf = buf.trim_right().to_string();

		let lines = if n == 0 {
			None
		} else {
			Some(vec![buf])
		};

		Ok(CsaFileStream {
			reader:reader,
			lines:lines,
			current_pos:0,
		})
	}
}
impl CsaStream for CsaFileStream {
	fn next(&mut self) -> Result<Option<String>,CsaStreamReadError> {
		let read_next = match self.lines {
			Some(ref lines) if self.current_pos >= lines.len() as u32 => true,
			_ => false,
		};

		if read_next {
			self.current_pos = 0;

			let mut buf = String::new();
			let n = self.reader.read_line(&mut buf)?;

			if n == 0 {
				self.lines = None;
			} else {
				self.lines = Some(self.read_real_line(buf));
			}
		}

		match self.lines {
			None => Ok(None),
			Some(ref lines) => {
				let p = self.current_pos;
				self.current_pos += 1;
				Ok(Some(lines[p as usize].clone()))
			}
		}
	}
}
pub struct CsaParser<S> where S: CsaStream {
	st:S,
}
impl<S> CsaParser<S> where S: CsaStream {
	pub fn new(st:S) -> CsaParser<S> where S: CsaStream {
		CsaParser {
			st:st
		}
	}

	pub fn parse(&mut self) -> Result<Vec<CsaData>,CsaParserError> {
		let mut results:Vec<CsaData> = Vec::new();
		let mut comments:Vec<String> = Vec::new();
		let mut stage = Stage::Initial;

		let mut current = self.read_next(&mut comments)?;

		let mut version = None;
		let mut info = None;
		let mut banmen:[[KomaKind; 9]; 9] = [[KomaKind::Blank; 9]; 9];
		let mut msente:HashMap<MochigomaKind,u32> = HashMap::new();
		let mut mgote:HashMap<MochigomaKind,u32> = HashMap::new();
		let mut mvs:Vec<Move> = Vec::new();
		let mut elapsed:Vec<Option<u32>> = Vec::new();
		let mut end_state = None;

		while let Some(line) = current {
			if line.starts_with("V") && stage == Stage::Initial {
				stage = Stage::Version;
				version = Some(line.clone());
			} else if (line.starts_with("N+") || line.starts_with("N-") ||
						line.starts_with("$")) && stage == Stage::Version {
				stage = Stage::Info;

				if let None = info {
					info = Some(KifuInfo::new());
				}

				if let Some(ref mut info) = info {
					info.parse(&line)?;
				}
			} else if line.starts_with("PI") && stage >= Stage::Version && stage <= Stage::Info {
				stage = Stage::Position;
			} else if line.starts_with("P1") && stage >= Stage::Version && stage <= Stage::Info {
				stage = Stage::Position;
			} else if (line.starts_with("P+") || line.starts_with("P-")) &&
						 stage >= Stage::Version && stage <= Stage::Info {
				stage = Stage::Position;
			} else if (line.starts_with("+") ||
						line.starts_with("-")) && stage == Stage::Position {
				stage = Stage::Moves;
			} else if line.starts_with("%") && stage >= Stage::Position {
				stage = Stage::EndState;
				end_state = Some(EndState::try_from(line.to_string())?);
			} else if line == "/" && stage >= Stage::Position {
				stage = Stage::Initial;

				results.push(CsaData::new(version,
											info,
											Banmen(banmen),
											MochigomaCollections::Pair(msente,mgote),
											mvs,elapsed,end_state));
				version = None;
				info = None;
				banmen = [[KomaKind::Blank; 9]; 9];
				msente = HashMap::new();
				mgote = HashMap::new();
				mvs = Vec::new();
				elapsed = Vec::new();
				end_state = None;
			} else {
				return Err(CsaParserError::FormatError(String::from("Invalid csa format.")));
			}

			current = self.read_next(&mut comments)?;
		}

		if stage >= Stage::Position {
			results.push(CsaData::new(version,
										info,
										Banmen(banmen),
										MochigomaCollections::Pair(msente,mgote),
										mvs,elapsed,end_state));
		}

		Ok(results)
	}

	fn read_next(&mut self, comments:&mut Vec<String>) -> Result<Option<String>,CsaStreamReadError> {
		while let Some(ref line) = self.st.next()? {
			if line.starts_with("'") {
				comments.push(String::from(&line.as_str()[1..]));
			} else {
				return Ok(Some(line.clone()));
			}
		}

		Ok(None)
	}
}
#[derive(Clone, Copy, Eq, PartialOrd, PartialEq, Debug)]
enum Stage {
	Initial = 0,
	Version,
	Info,
	Position,
	Moves,
	EndState,
}
#[derive(Debug)]
pub struct CsaData {
	pub version:Option<String>,
	pub kifu_info:Option<KifuInfo>,
	pub initial_position:Banmen,
	pub initial_mochigoma:MochigomaCollections,
	pub moves:Vec<Move>,
	pub elapsed:Vec<Option<u32>>,
	pub end_state:Option<EndState>,
}
impl CsaData {
	pub fn new(version:Option<String>,
				kifu_info:Option<KifuInfo>,
				banmen:Banmen,
				mochigoma:MochigomaCollections,
				mvs:Vec<Move>, elapsed:Vec<Option<u32>>,
				end_state:Option<EndState>) -> CsaData {
		CsaData {
			version:version,
			kifu_info:kifu_info,
			initial_position:banmen,
			initial_mochigoma:mochigoma,
			moves:mvs,
			elapsed:elapsed,
			end_state:end_state,
		}
	}
}
#[derive(Debug)]
pub struct KifuInfo {
	pub sente_name:Option<String>,
	pub gote_name:Option<String>,
	pub event:Option<String>,
	pub site:Option<String>,
	pub start_time:Option<String>,
	pub end_time:Option<String>,
	pub time_limit:Option<(u32,Option<u32>)>,
	pub opening:Option<String>,
}
impl KifuInfo {
	pub fn new() -> KifuInfo {
		KifuInfo {
			sente_name:None,
			gote_name:None,
			event:None,
			site:None,
			start_time:None,
			end_time:None,
			time_limit:None,
			opening:None,
		}
	}

	pub fn parse(&mut self, line:&String) -> Result<(),CsaParserError> {
		if line.starts_with("N+") {
			self.sente_name = Some(String::from(&line.as_str()[2..]));
		} else if line.starts_with("N-") {
			self.gote_name = Some(String::from(&line.as_str()[2..]));
		} else if line.starts_with("$EVENT:") {
			self.event = Some(String::from(&line.as_str()[7..]));
		} else if line.starts_with("$SITE:") {
			self.site = Some(String::from(&line.as_str()[6..]));
		} else if line.starts_with("$START_TIME:") {
			self.start_time = Some(String::from(&line.as_str()[12..]));
		} else if line.starts_with("$END_TIME:") {
			self.end_time = Some(String::from(&line.as_str()[10..]));
		} else if line.starts_with("$TIME_LIMIT:") {
			let t = String::from(&line.as_str()[12..]);
			let mut chars = t.chars();

			let mut hh = String::new();
			let mut mm = String::new();

			for _ in 0..2 {
				match chars.next() {
					None => {
						return Err(CsaParserError::FormatError(String::from(
							"Invalid csa info format of timelimit."
						)));
					},
					Some(c) => {
						hh.push(c);
					}
				}
			}

			let h:u32 = hh.parse()?;

			let delimiter = chars.next();

			if delimiter == None {
				return Err(CsaParserError::FormatError(String::from(
					"Invalid csa info format of timelimit."
				)));
			}

			if let Some(c) = delimiter {
				if c != ':' {
					return Err(CsaParserError::FormatError(String::from(
						"Invalid csa info format of timelimit."
					)));
				}
			}

			for _ in 0..2 {
				match chars.next() {
					None => {
						return Err(CsaParserError::FormatError(String::from(
							"Invalid csa info format of timelimit."
						)));
					},
					Some(c) => {
						mm.push(c);
					}
				}
			}

			let m:u32 = mm.parse()?;

			let s = match chars.next() {
				None => None,
				Some('+') => {
					let mut ss = String::new();

					for _ in 0..2 {
						match chars.next() {
							None => {
								return Err(CsaParserError::FormatError(String::from(
									"Invalid csa info format of timelimit."
								)));
							},
							Some(c) => {
								ss.push(c);
							}
						}
					}

					let s = ss.parse()?;

					match chars.next() {
						None => Some(s),
						Some(_) => {
							return Err(CsaParserError::FormatError(String::from(
								"Invalid csa info format of timelimit."
							)));
						}
					}
				},
				_ => {
					return Err(CsaParserError::FormatError(String::from(
						"Invalid csa info format of timelimit."
					)));
				}
			};

			self.time_limit = Some((h * 60 + m, s));
		} else if line.starts_with("$OPENING:") {
			self.opening = Some(String::from(&line.as_str()[9..]));
		} else {
			return Err(CsaParserError::FormatError(String::from(
				"Invalid csa info format."
			)));
		}

		Ok(())
	}
}
#[derive(Clone, Copy, Eq, PartialOrd, PartialEq, Debug)]
pub enum EndState {
	Toryo = 0, // 投了
	Chudan, // 中断
	Sennichite, // 千日手
	TimeUp, // 手番側が時間切れで負け
	IllegalMove, // 手番側の反則負け、反則の内容はコメントで記録する
	SIllegalAction, // 先手(下手)の反則行為により、後手(上手)の勝ち
	GIllegalAction, // 後手(上手)の反則行為により、先手(下手)の勝ち
	Jishogi, // 持将棋
	Kachi, // (入玉で)勝ちの宣言
	Hikiwake, // (入玉で)引き分けの宣言
	Matta, // 待った
	Tsumi, // 詰み
	Fuzumi, // 不詰
	Error, // エラー
}
impl TryFrom<String,TypeConvertError<String>> for EndState {
	fn try_from(kind:String) -> Result<EndState,TypeConvertError<String>> {
		Ok(match &*kind {
			"%TORYO" => EndState::Toryo,
			"%CHUDAN" => EndState::Chudan,
			"%SENNICHITE" => EndState::Sennichite,
			"%TIME_UP" => EndState::TimeUp,
			"%ILLEGAL_MOVE" => EndState::IllegalMove,
			"%+ILLEGAL_ACTION" => EndState::SIllegalAction,
			"%-ILLEGAL_ACTION" => EndState::GIllegalAction,
			"%JISHOGI" => EndState::Jishogi,
			"%KACHI" => EndState::Kachi,
			"%HIKIWAKE" => EndState::Hikiwake,
			"%MATTA" => EndState::Matta,
			"%TSUMI" => EndState::Tsumi,
			"%FUZUMI" => EndState::Fuzumi,
			"%ERROR" => EndState::Error,
			_ => {
				return Err(TypeConvertError::SyntaxError(String::from("Invalid end state.")));
			}
		})
	}
}