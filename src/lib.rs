extern crate usiagent;

pub mod error;

use std::io;
use std::io::BufReader;
use std::io::BufRead;
use std::fs::File;
use std::fs::OpenOptions;
use std::path::Path;
use std::collections::HashMap;
use std::str::Chars;
use std::slice::Iter;
use std::ops::Index;

use usiagent::shogi::*;
use usiagent::shogi::KomaKind::{
	SFu,
	SKyou,
	SKei,
	SGin,
	SKin,
	SKaku,
	SHisha,
	SOu,
	SFuN,
	SKyouN,
	SKeiN,
	SGinN,
	SKakuN,
	SHishaN,
	GFu,
	GKyou,
	GKei,
	GGin,
	GKin,
	GKaku,
	GHisha,
	GOu,
	GFuN,
	GKyouN,
	GKeiN,
	GGinN,
	GKakuN,
	GHishaN,
	Blank
};
use usiagent::rule::*;

use error::*;

pub trait CsaStream {
	fn next(&mut self) -> Result<Option<String>,CsaStreamReadError>;
	fn read_real_line(l:String) -> Vec<String> {
		if l.starts_with('\'') {
			vec![l]
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
	pub fn new<P>(file:P) -> Result<CsaFileStream,io::Error> where P: AsRef<Path> {
		let mut reader = BufReader::new(OpenOptions::new().read(true).create(false).open(file)?);
		let mut buf = String::new();
		let n = reader.read_line(&mut buf)?;

		let r: &[_] = &['\0'];

		buf = buf.trim_end().trim_end_matches(r).to_string();

		let lines = if n == 0 {
			None
		} else {
			Some(CsaFileStream::read_real_line(buf))
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
			let mut n = self.reader.read_line(&mut buf)?;

			let r: &[_] = &['\0'];

			buf = buf.trim_end().trim_end_matches(r).to_string();

			while n > 0 && buf == "" {
				buf.clear();
				n = self.reader.read_line(&mut buf)?;
				buf = buf.trim_end().trim_end_matches(r).to_string();
			}

			if n == 0 {
				self.lines = None;
			} else {
				self.lines = Some(CsaFileStream::read_real_line(buf));
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
pub struct CsaStringReader {

}
impl CsaStringReader {
	pub fn new() -> CsaStringReader {
		CsaStringReader {

		}
	}

	pub fn read(&mut self,chars:&mut Chars,len:u32) -> Result<String,CsaParserError> {
		let mut s = String::new();

		for _ in 0..len {
			match chars.next() {
				None => {
					return Err(CsaParserError::FormatError(String::from(
						"Invalid csa format, Could not read the specified length."
					)));
				},
				Some(c) => {
					s.push(c);
				}
			}
		}

		Ok(s)
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

	fn read_next_lines<F>(&mut self,lines:&mut Vec<String>, comments:&mut Vec<String>,cond:F)
		-> Result<Option<String>,CsaStreamReadError> where F: Fn(&String) -> bool {

		let mut current = self.read_next(comments)?;

		let mut l = match current {
			Some(ref l) => {
				l.clone()
			},
			None => {
				return Ok(current);
			}
		};

		while cond(&l) {
			lines.push(l.clone());

			current = self.read_next(comments)?;

			l = match current {
				Some(ref l) => {
					l.clone()
				},
				None => {
					break;
				}
			};
		}

		Ok(current)
	}

	pub fn parse(&mut self) -> Result<Vec<CsaData>,CsaParserError> {
		let mut results:Vec<CsaData> = Vec::new();
		let mut comments:Vec<String> = Vec::new();
		let mut stage = Stage::Initial;

		let mut current = self.read_next(&mut comments)?;

		let mut version = None;
		let mut info = None;
		let banmen:[[KomaKind; 9]; 9] = [[KomaKind::Blank; 9]; 9];
		let mut teban = Teban::Sente;
		let mut banmen = Banmen(banmen);
		let msente:HashMap<MochigomaKind,u32> = HashMap::new();
		let mgote:HashMap<MochigomaKind,u32> = HashMap::new();
		let mut mc = MochigomaCollections::Pair(msente,mgote);
		let mut mvs:CsaMoves = CsaMoves::new();
		let mut end_state = None;

		while let Some(line) = current {
			if line.starts_with("V") && stage == Stage::Initial {
				stage = Stage::Version;
				version = Some(String::from(&line.as_str()[1..]));
				current = self.read_next(&mut comments)?;
			} else if (line.starts_with("N+") ||
						line.starts_with("N-") ||
						line.starts_with("$")) &&
						(stage == Stage::Version || stage == Stage::Info) {
				stage = Stage::Info;

				if let None = info {
					info = Some(KifuInfo::new());
				}

				if let Some(ref mut info) = info {
					info.parse(&line)?;
				}
				current = self.read_next(&mut comments)?;
			} else if line.starts_with("PI") && stage >= Stage::Version && stage <= Stage::Info {
				stage = Stage::Position;
				let (b,m) = CsaPositionParser::new().parse(vec![line.clone()])?;
				banmen = b;
				mc = m;
				current = self.read_next(&mut comments)?;
			} else if (line.starts_with("P1") ||
						line.starts_with("P+") ||
						line.starts_with("P-")) &&
							stage >= Stage::Version && stage <= Stage::Info {
				stage = Stage::Position;

				let mut lines:Vec<String> = Vec::new();

				let l = line;
				lines.push(l.clone());

				current = self.read_next_lines(&mut lines,&mut comments,|s| {
					s.starts_with("P")
				})?;

				let (b,m) = CsaPositionParser::new().parse(lines)?;
				banmen = b;
				mc = m;
			} else if (line == "+" || line == "-") && stage == Stage::Position {
				stage = Stage::Moves;

				let mut lines:Vec<String> = Vec::new();

				let l = line;
				lines.push(l.clone());

				current = self.read_next_lines(&mut lines,&mut comments,|s| {
					s.starts_with("+") || s.starts_with("-") || s.starts_with("T") || s.starts_with("%")
				})?;

				let (t,m,s) = CsaMovesParser::new().parse(lines,&banmen)?;

				teban = t;
				mvs = m;
				end_state = s;
			} else if line == "/" && stage >= Stage::Position {
				stage = Stage::Initial;

				results.push(CsaData::new(version,
											info,
											teban,
											banmen,
											mc,
											mvs,
											end_state,
											comments));
				version = None;
				info = None;
				banmen = Banmen([[KomaKind::Blank; 9]; 9]);
				mc = MochigomaCollections::Pair(HashMap::new(),HashMap::new());
				mvs = CsaMoves::new();
				end_state = None;
				comments = Vec::new();
				current = self.read_next(&mut comments)?;
			} else {
				return Err(CsaParserError::FormatError(String::from("Invalid csa format.")));
			}
		}

		if stage >= Stage::Position {
			results.push(CsaData::new(version,
										info,
										teban,
										banmen,
										mc,
										mvs,end_state,comments));
			Ok(results)
		} else {
			Err(CsaParserError::FormatError(String::from("Invalid csa format.")))
		}
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
}
#[derive(Debug)]
pub struct CsaData {
	pub version:Option<String>,
	pub kifu_info:Option<KifuInfo>,
	pub teban_at_start:Teban,
	pub initial_position:Banmen,
	pub initial_mochigoma:MochigomaCollections,
	pub moves:CsaMoves,
	pub end_state:Option<EndState>,
	pub comments:Vec<String>,
}
impl CsaData {
	pub fn new(version:Option<String>,
				kifu_info:Option<KifuInfo>,
				teban:Teban,
				banmen:Banmen,
				mochigoma:MochigomaCollections,
				mvs:CsaMoves,
				end_state:Option<EndState>,
				comments:Vec<String>) -> CsaData {
		CsaData {
			version:version,
			kifu_info:kifu_info,
			teban_at_start:teban,
			initial_position:banmen,
			initial_mochigoma:mochigoma,
			moves:mvs,
			end_state:end_state,
			comments:comments,
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
		let mut reader = CsaStringReader::new();

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

			let hh = reader.read(&mut chars, 2)?;


			let delimiter = chars.next();

			if let None = delimiter {
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

			let mm = reader.read(&mut chars, 2)?;

			let h:u32 = hh.parse()?;
			let m:u32 = mm.parse()?;

			let s = match chars.next() {
				None => None,
				Some('+') => {
					let ss = reader.read(&mut chars, 2)?;

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
pub trait TryFromCsa<T> where Self: Sized {
	fn try_from_csa(kind:T) -> Result<Self,CsaParserError>;
}
impl<'a> TryFromCsa<&'a String> for MochigomaKind {
	fn try_from_csa(kind:&'a String) -> Result<MochigomaKind,CsaParserError> {
		Ok(match kind.as_str() {
			"FU" | "TO" => MochigomaKind::Fu,
			"KY" | "NY" => MochigomaKind::Kyou,
			"KE" | "NK" => MochigomaKind::Kei,
			"GI" | "NG" => MochigomaKind::Gin,
			"KI" => MochigomaKind::Kin,
			"KA" | "UM" => MochigomaKind::Kaku,
			"HI" | "RY" => MochigomaKind::Hisha,
			_ => {
				return Err(CsaParserError::FormatError(String::from(
					"Invalid csa position format."
				)));
			}
		})
	}
}
impl<'a> TryFromCsa<(Teban,&'a String)> for KomaKind {
	fn try_from_csa(s:(Teban,&'a String)) -> Result<KomaKind,CsaParserError> {
		let (teban,kind) = s;

		Ok(match kind.as_str() {
			"FU" | "KY" | "KE" | "GI" | "KI" | "KA" | "HI" => {
				KomaKind::from((teban,MochigomaKind::try_from_csa(kind)?))
			},
			"OU" if teban == Teban::Sente => SOu,
			"OU" => GOu,
			"TO" if teban == Teban::Sente => SFuN,
			"TO" => GFuN,
			"NY" if teban == Teban::Sente => SKyouN,
			"NY" => GKyouN,
			"NK" if teban == Teban::Sente => SKeiN,
			"NK" => GKeiN,
			"NG" if teban == Teban::Sente => SGinN,
			"NG" => GGinN,
			"UM" if teban == Teban::Sente => SKakuN,
			"UM" => GKakuN,
			"RY" if teban == Teban::Sente => SHishaN,
			"RY" => GHishaN,
			_ => {
				return Err(CsaParserError::FormatError(String::from(
					"Invalid csa position format."
				)));
			}
		})
	}
}
impl<'a> TryFromCsa<&'a String> for EndState {
	fn try_from_csa(kind:&'a String) -> Result<EndState,CsaParserError> {
		Ok(match kind.as_str() {
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
				return Err(CsaParserError::FormatError(String::from(
					"Invalid end state."))
				);
			}
		})
	}
}
struct CsaPositionParser {

}
impl CsaPositionParser {
	pub fn new() -> CsaPositionParser {
		CsaPositionParser {

		}
	}

	fn create_error(&self) -> CsaParserError {
		CsaParserError::FormatError(String::from(
			"Invalid csa position format."
		))
	}

	pub fn parse(&mut self, lines:Vec<String>)
		-> Result<(Banmen,MochigomaCollections),CsaParserError> {

		if lines.len() == 0 {
			return Err(CsaParserError::InvalidStateError(String::from(
				"lines is empty."
			)));
		}

		let mut reader = CsaStringReader::new();

		if lines[0].starts_with("PI") {
			let initial_banmen = BANMEN_START_POS.clone();
			let mut initial_banmen = initial_banmen.0;
			let mut chars = lines[0].chars();
			chars.next();
			chars.next();

			while let Some(c) = chars.next() {
				let x = c;

				let y = match chars.next() {
					None => {
						return Err(self.create_error());
					},
					Some(y) => {
						y
					}
				};

				if x < '1' || x > '9' || y < '1' || y > '9' {
					return Err(self.create_error());
				}

				let x = x as usize - '0' as usize;
				let y = y as usize - '0' as usize;

				let kind = reader.read(&mut chars, 2)?;

				let k = initial_banmen[y-1][9-x];

				match &*kind {
					"FU" if k == SFu || k == GFu => {
						initial_banmen[y-1][9-x] = Blank;
					},
					"KY" if k == SKyou || k == GKyou => {
						initial_banmen[y-1][9-x] = Blank;
					},
					"KE" if k == SKei || k == GKei => {
						initial_banmen[y-1][9-x] = Blank;
					},
					"GI" if k == SGin || k == GGin => {
						initial_banmen[y-1][9-x] = Blank;
					},
					"KI" if k == SKin || k == GKin => {
						initial_banmen[y-1][9-x] = Blank;
					},
					"KA" if k == SKaku || k == GKaku => {
						initial_banmen[y-1][9-x] = Blank;
					},
					"HI" if k == SHisha || k == GHisha => {
						initial_banmen[y-1][9-x] = Blank;
					},
					_ => {
						return Err(self.create_error());
					}
				}
			}

			Ok((Banmen(initial_banmen),MochigomaCollections::Empty))
		} else if lines[0].starts_with("P1") {
			let mut initial_banmen:[[KomaKind; 9]; 9] = [[KomaKind::Blank; 9]; 9];
			let mut ms = Rule::filled_mochigoma_hashmap();
			let mut mg = Rule::filled_mochigoma_hashmap();

			let mut sou_count = 1;
			let mut gou_count = 1;

			for i in 0..9 {
				let line_number = (i + '1' as u8) as char;

				let mut chars = lines[i as usize].chars();

				match chars.next() {
					None => {
						return Err(self.create_error());
					},
					Some(c) if c != 'P' => {
						return Err(self.create_error());
					},
					_ => (),
				}

				match chars.next() {
					None => {
						return Err(self.create_error());
					},
					Some(c) if c != line_number => {
						return Err(self.create_error());
					},
					_ => (),
				}

				for j in 0..9 {
					match chars.next() {
						None => {
							return Err(self.create_error());
						},
						Some(c) if c == '+' || c == '-' => {
							let teban = match c {
								'+' => Teban::Sente,
								'-' => Teban::Gote,
								_ => {
									return Err(self.create_error());
								}
							};

							let kind = reader.read(&mut chars, 2)?;

							if kind == "OU" {
								match teban {
									Teban::Sente => {
										if sou_count == 1 {
											sou_count -= 1;
											initial_banmen[i as usize][j as usize] = SOu;
										} else {
											return Err(self.create_error());
										}
									},
									Teban::Gote => {
										if gou_count == 1 {
											gou_count -= 1;
											initial_banmen[i as usize][j as usize] = GOu;
										} else {
											return Err(self.create_error());
										}
									}
								}
							} else {
								let k = MochigomaKind::try_from_csa(&kind)?;

								match teban {
									Teban::Sente => {
										let c = match ms.get(&k) {
											None | Some(&0)=> {
												return Err(self.create_error());
											},
											Some(c) => {
												c - 1
											}
										};

										ms.insert(k, c);
									},
									Teban::Gote => {
										let c = match mg.get(&k) {
											None | Some(&0)=> {
												return Err(self.create_error());
											},
											Some(c) => {
												c - 1
											}
										};

										mg.insert(k, c);
									}
								}

								initial_banmen[i as usize][j as usize] = KomaKind::try_from_csa((teban,&kind))?;
							}
						},
						_ => {
							chars.next();
							chars.next();
						}
					}
				}
			}

			Ok((Banmen(initial_banmen),MochigomaCollections::Pair(ms,mg)))
		} else if lines[0].starts_with("P+") || lines[0].starts_with("P-") {
			let mut initial_banmen:[[KomaKind; 9]; 9] = [[KomaKind::Blank; 9]; 9];
			let mut ms = Rule::filled_mochigoma_hashmap();
			let mut mg = Rule::filled_mochigoma_hashmap();

			let mut sou_count = 1;
			let mut gou_count = 1;

			for i in 0..lines.len() {
				let line = &lines[i];

				let teban = if line.starts_with("P+") {
					Teban::Sente
				} else if line.starts_with("P-") {
					Teban::Gote
				} else {
					return Err(self.create_error());
				};

				let mut chars = line.chars();

				chars.next();
				chars.next();

				while let Some(c) = chars.next() {
					let x = c;

					let y = match chars.next() {
						None => {
							return Err(self.create_error());
						},
						Some(y) => {
							y
						}
					};

					if x == '0' && y == '0' {
						if i < lines.len() - 1 {
							return Err(self.create_error());
						}

						if chars.next() == Some('A') &&
							chars.next() == Some('L') &&
							chars.next() == None {

							match teban {
								Teban::Sente => {
									for m in &MOCHIGOMA_KINDS {
										let c = *mg.get(m).unwrap_or(&0);
										mg.insert(*m, 0);
										let c = *ms.get(m).unwrap_or(&0) + c;
										ms.insert(*m,c);
									}
								},
								Teban::Gote => {
									for m in &MOCHIGOMA_KINDS {
										let c = *ms.get(m).unwrap_or(&0);
										ms.insert(*m, 0);
										let c = *mg.get(m).unwrap_or(&0) + c;
										mg.insert(*m,c);
									}
								}
							}
						} else {
							return Err(self.create_error());
						}
					} else {
						if x < '1' || x > '9' || y < '1' || y > '9' {
							return Err(self.create_error());
						}

						let x = x as usize - '0' as usize;
						let y = y as usize - '0' as usize;

						let kind = reader.read(&mut chars, 2)?;

						let k = initial_banmen[y-1][9-x];

						if k != Blank {
							return Err(self.create_error());
						}

						if kind == "OU" {
							match teban {
								Teban::Sente => {
									if sou_count == 1 {
										sou_count -= 1;
										initial_banmen[y-1][9-x] = SOu;
									} else {
										return Err(self.create_error());
									}
								},
								Teban::Gote => {
									if gou_count == 1 {
										gou_count -= 1;
										initial_banmen[y-1][9-x] = GOu;
									} else {
										return Err(self.create_error());
									}
								}
							}
						} else {
							let k = MochigomaKind::try_from_csa(&kind)?;

							match teban {
								Teban::Sente => {
									let c = match ms.get(&k) {
										None | Some(&0)=> {
											return Err(self.create_error());
										},
										Some(c) => {
											c - 1
										}
									};

									ms.insert(k, c);
								},
								Teban::Gote => {
									let c = match mg.get(&k) {
										None | Some(&0)=> {
											return Err(self.create_error());
										},
										Some(c) => {
											c - 1
										}
									};

									mg.insert(k, c);
								}
							}

							initial_banmen[y-1][9-x] = KomaKind::try_from_csa((teban,&kind))?;
						}
					}
				}
			}

			Ok((Banmen(initial_banmen),MochigomaCollections::Pair(ms,mg)))
		} else {
			Err(self.create_error())
		}
	}
}
struct CsaMovesParser {

}
impl CsaMovesParser {
	pub fn new() -> CsaMovesParser {
		CsaMovesParser {

		}
	}

	fn create_error(&self) -> CsaParserError {
		CsaParserError::FormatError(String::from(
			"Invalid csa moves format."
		))
	}

	fn parse_elpsed(&self,lines:&Vec<String>,i:&mut usize,len:usize) -> Result<Option<i32>,CsaParserError> {
		*i += 1;

		if *i < len && lines[*i].starts_with("T") {
			let line = &lines[*i];

			let s = String::from(&line.as_str()[1..]);
			let s:i32 = s.parse()?;

			*i += 1;

			Ok(Some(s))
		} else {
			Ok(None)
		}
	}

	pub fn parse(&mut self, lines:Vec<String>,banmen:&Banmen)
		-> Result<(Teban,CsaMoves,Option<EndState>),CsaParserError> {

		if lines.len() == 0 {
			return Err(CsaParserError::InvalidStateError(String::from(
				"lines is empty."
			)));
		}

		let mut teban = match &*lines[0] {
			"+" => Teban::Sente,
			"-" => Teban::Gote,
			_ => {
				return Err(self.create_error());
			}
		};

		let teban_at_start = teban;

		let mut banmen = match banmen {
			Banmen(ref kinds) => kinds.clone()
		};

		let mut mvs:CsaMoves = CsaMoves::new();

		let mut reader = CsaStringReader::new();

		let mut i = 1;
		let len = lines.len();
		let mut moveend = false;
		let mut end_state = None;

		while i < len {
			let line = &lines[i];

			if line == "%KACHI" && !moveend {
				mvs.push(CsaMove::Kachi(self.parse_elpsed(&lines,&mut i,len)?))?;
				moveend = true;
			} else if line == "%HIKIWAKE" && !moveend {
				mvs.push(CsaMove::Hikiwake(self.parse_elpsed(&lines,&mut i,len)?))?;
				moveend = true;
			} else if line.starts_with("+") || line.starts_with("-") {
				match teban {
					Teban::Sente if !line.starts_with("+") => {
						return Err(self.create_error());
					},
					Teban::Gote if !line.starts_with("-") => {
						return Err(self.create_error());
					},
					_ => ()
				}

				let mut chars = line.chars();
				chars.next();

				let (sx,sy) = match chars.next() {
					None => {
						return Err(self.create_error());
					},
					Some(sx) => {
						let sx = sx as u32 - '0' as u32;

						let sy = match chars.next() {
							None => {
								return Err(self.create_error());
							},
							Some(sy) => {
								sy as u32 - '0' as u32
							}
						};

						(sx,sy)
					}
				};

				let (dx,dy) = match chars.next() {
					None => {
						return Err(self.create_error());
					},
					Some(dx) => {
						let dx = dx as u32 - '0' as u32;

						let dy = match chars.next() {
							None => {
								return Err(self.create_error());
							},
							Some(dy) => {
								dy as u32 - '0' as u32
							}
						};

						(dx,dy)
					}
				};

				let kind = reader.read(&mut chars, 2)?;

				if sx == 0 && sy == 0 && dx >= 1 && dx <= 9 && dy >= 1 && dy <= 9 {
					let k = MochigomaKind::try_from_csa(&kind)?;

					mvs.push(CsaMove::Move(Move::Put(k,KomaDstPutPosition(dx,dy)),
												self.parse_elpsed(&lines,&mut i,len)?))?;

					let dx = dx as usize;
					let dy = dy as usize;
					banmen[dy-1][9-dx] = KomaKind::from((teban,k));
				} else {
					if sx < 1 || sx > 9 || sy < 1 || sy > 9 {
						return Err(self.create_error());
					}

					if dx < 1 || dx > 9 || dy < 1 || dy > 9 {
						return Err(self.create_error());
					}

					let k = KomaKind::try_from_csa((teban,&kind))?;

					let sx = sx as usize;
					let sy = sy as usize;
					let dx = dx as usize;
					let dy = dy as usize;

					let n = match k {
						SFuN |
							SKyouN |
							SKeiN |
							SGinN |
							SKakuN |
							SHishaN if banmen[sy-1][9-sx] != k => {

							true
						},
						GFuN |
							GKyouN |
							GKeiN |
							GGinN |
							GKakuN |
							GHishaN if banmen[sy-1][9-sx] != k => {

							true
						},
						_ => false,
					};

					mvs.push(CsaMove::Move(Move::To(
						KomaSrcPosition(sx as u32,sy as u32),
						KomaDstToPosition(dx as u32,dy as u32,n)
					),self.parse_elpsed(&lines,&mut i,len)?))?;

					banmen[sy-1][9-sx] = Blank;
					banmen[dy-1][9-dx] = k;
				}
			} else if line.starts_with("%") {
				end_state = Some(EndState::try_from_csa(line)?);
				i += 1;
			} else {
				return Err(self.create_error());
			}

			teban = teban.opposite();
		}
		Ok((teban_at_start,mvs,end_state))
	}
}
#[derive(Clone, Copy, Eq, PartialOrd, PartialEq, Debug)]
pub enum CsaMove {
	Move(Move,Option<i32>),
	Kachi(Option<i32>),
	Hikiwake(Option<i32>),
}
#[derive(Debug)]
pub struct CsaMoves {
	moves:Vec<CsaMove>,
}
impl CsaMoves {
	pub fn new() -> CsaMoves {
		CsaMoves {
			moves:Vec::new()
		}
	}

	pub fn iter(&self) -> Iter<CsaMove> {
		self.moves.iter()
	}

	pub fn len(&self) -> usize {
		self.moves.len()
	}

	pub fn push(&mut self,m:CsaMove) -> Result<usize,CsaStateError> {
		let len = self.len();

		if len == 0 {
			self.moves.push(m);
		} else {
			match self.moves[len-1] {
				CsaMove::Kachi(_) | CsaMove::Hikiwake(_) => {
					match m {
						CsaMove::Kachi(_) | CsaMove::Hikiwake(_) => {
							return Err(CsaStateError::InvalidStateError(String::from(
								"Only one special move can store."
							)));
						},
						_ => (),
					}
				},
				_ => {

				}
			}

			self.moves.push(m);
		}

		Ok(len)
	}
}
impl<'a> IntoIterator for &'a CsaMoves {
	type Item = &'a CsaMove;
	type IntoIter = Iter<'a,CsaMove>;

	fn into_iter(self) -> Self::IntoIter {
		self.iter()
	}
}
impl Index<usize> for CsaMoves {
	type Output = CsaMove;

	fn index(&self,i:usize) -> &CsaMove {
		&self.moves[i]
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
