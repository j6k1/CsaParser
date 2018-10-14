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
use usiagent::shogi::KomaKind::{SFu,SKyou,SKei,SGin,SKin,SKaku,SHisha,SOu,GFu,GKyou,GKei,GGin,GKin,GKaku,GHisha,GOu,Blank};
use usiagent::rule::*;
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
		let banmen:[[KomaKind; 9]; 9] = [[KomaKind::Blank; 9]; 9];
		let mut banmen = Banmen(banmen);
		let msente:HashMap<MochigomaKind,u32> = HashMap::new();
		let mgote:HashMap<MochigomaKind,u32> = HashMap::new();
		let mut mc = MochigomaCollections::Pair(msente,mgote);
		let mut mvs:Vec<Move> = Vec::new();
		let mut elapsed:Vec<Option<u32>> = Vec::new();
		let mut end_state = None;

		while let Some(line) = current {
			if line.starts_with("V") && stage == Stage::Initial {
				stage = Stage::Version;
				version = Some(line.clone());
				current = self.read_next(&mut comments)?;
			} else if (line.starts_with("N+") || line.starts_with("N-") ||
						line.starts_with("$")) && stage == Stage::Version {
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

				let mut l = line;
				lines.push(l.clone());

				current = self.read_next(&mut comments)?;

				while l.starts_with("P") {
					lines.push(l.clone());

					current = self.read_next(&mut comments)?;

					l = match current {
						Some(ref l) => {
							l.clone()
						},
						None => {
							break;
						}
					};
				}

				let (b,m) = CsaPositionParser::new().parse(lines)?;
				banmen = b;
				mc = m;
			} else if (line == "+" || line == "-") && stage == Stage::Position {
				stage = Stage::Moves;
				current = self.read_next(&mut comments)?;
			} else if line.starts_with("%") && stage >= Stage::Position {
				stage = Stage::EndState;
				end_state = Some(EndState::try_from(line.to_string())?);
				current = self.read_next(&mut comments)?;
			} else if line == "/" && stage >= Stage::Position {
				stage = Stage::Initial;

				results.push(CsaData::new(version,
											info,
											banmen,
											mc,
											mvs,
											elapsed,
											end_state,
											comments));
				version = None;
				info = None;
				banmen = Banmen([[KomaKind::Blank; 9]; 9]);
				mc = MochigomaCollections::Pair(HashMap::new(),HashMap::new());
				mvs = Vec::new();
				elapsed = Vec::new();
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
										banmen,
										mc,
										mvs,elapsed,end_state,comments));
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
	pub comments:Vec<String>,
}
impl CsaData {
	pub fn new(version:Option<String>,
				kifu_info:Option<KifuInfo>,
				banmen:Banmen,
				mochigoma:MochigomaCollections,
				mvs:Vec<Move>, elapsed:Vec<Option<u32>>,
				end_state:Option<EndState>,
				comments:Vec<String>) -> CsaData {
		CsaData {
			version:version,
			kifu_info:kifu_info,
			initial_position:banmen,
			initial_mochigoma:mochigoma,
			moves:mvs,
			elapsed:elapsed,
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

	pub fn parse(&mut self, lines:Vec<String>) -> Result<(Banmen,MochigomaCollections),CsaParserError> {
		if lines[0].starts_with("P1") {
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

				let mut kind = String::new();

				for _ in 0..2 {
					match chars.next() {
						None => {
							return Err(self.create_error());
						},
						Some(c) => {
							kind.push(c);
						}
					}
				}

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
			let mut ms:HashMap<MochigomaKind,u32> = HashMap::new();
			let mut mg:HashMap<MochigomaKind,u32> = HashMap::new();

			ms.insert(MochigomaKind::Fu, 9);
			ms.insert(MochigomaKind::Kyou, 2);
			ms.insert(MochigomaKind::Kei, 2);
			ms.insert(MochigomaKind::Gin, 2);
			ms.insert(MochigomaKind::Kin, 2);
			ms.insert(MochigomaKind::Kaku, 1);
			ms.insert(MochigomaKind::Hisha, 1);

			mg.insert(MochigomaKind::Fu, 9);
			mg.insert(MochigomaKind::Kyou, 2);
			mg.insert(MochigomaKind::Kei, 2);
			mg.insert(MochigomaKind::Gin, 2);
			mg.insert(MochigomaKind::Kin, 2);
			mg.insert(MochigomaKind::Kaku, 1);
			mg.insert(MochigomaKind::Hisha, 1);

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

							let mut kind = String::new();

							for _ in 0..2 {
								match chars.next() {
									None => {
										return Err(self.create_error());
									},
									Some(c) => {
										kind.push(c);
									}
								}
							}

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
								let k = match &*kind {
									"Fu" => MochigomaKind::Fu,
									"KY" => MochigomaKind::Kyou,
									"KE" => MochigomaKind::Kei,
									"GI" => MochigomaKind::Gin,
									"KI" => MochigomaKind::Kin,
									"KA" => MochigomaKind::Kaku,
									"HI" => MochigomaKind::Hisha,
									_ => {
										return Err(self.create_error());
									}
								};

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

								initial_banmen[i as usize][j as usize] = KomaKind::from((teban,k));
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
			let mut ms:HashMap<MochigomaKind,u32> = HashMap::new();
			let mut mg:HashMap<MochigomaKind,u32> = HashMap::new();

			ms.insert(MochigomaKind::Fu, 9);
			ms.insert(MochigomaKind::Kyou, 2);
			ms.insert(MochigomaKind::Kei, 2);
			ms.insert(MochigomaKind::Gin, 2);
			ms.insert(MochigomaKind::Kin, 2);
			ms.insert(MochigomaKind::Kaku, 1);
			ms.insert(MochigomaKind::Hisha, 1);

			mg.insert(MochigomaKind::Fu, 9);
			mg.insert(MochigomaKind::Kyou, 2);
			mg.insert(MochigomaKind::Kei, 2);
			mg.insert(MochigomaKind::Gin, 2);
			mg.insert(MochigomaKind::Kin, 2);
			mg.insert(MochigomaKind::Kaku, 1);
			mg.insert(MochigomaKind::Hisha, 1);

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

						let mut kind = String::new();

						for _ in 0..2 {
							match chars.next() {
								None => {
									return Err(self.create_error());
								},
								Some(c) => {
									kind.push(c);
								}
							}
						}

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
							let k = match &*kind {
								"Fu" => MochigomaKind::Fu,
								"KY" => MochigomaKind::Kyou,
								"KE" => MochigomaKind::Kei,
								"GI" => MochigomaKind::Gin,
								"KI" => MochigomaKind::Kin,
								"KA" => MochigomaKind::Kaku,
								"HI" => MochigomaKind::Hisha,
								_ => {
									return Err(self.create_error());
								}
							};

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

							initial_banmen[y-1][9-x] = KomaKind::from((teban,k));
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