extern crate usiagent;

pub mod error;

use std::io;
use std::io::BufReader;
use std::io::BufRead;
use std::fs::File;
use std::fs::OpenOptions;

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

	pub fn parse() -> Result<Vec<CsaData>,CsaParserError> {
		let mut results:Vec<CsaData> = Vec::new();

		Ok(results)
	}
}
#[derive(Debug)]
pub struct CsaData {
	pub version:Option<String>,
	pub kifu_info:Option<KifuInfo>,
	pub initial_position:Banmen,
	pub initial_mochigoma:MochigomaCollections,
	pub moves:Vec<Move>,
	pub elapsed:Vec<u32>,
	pub end_state:Option<EndState>,
}
impl CsaData {
	pub fn new(banmen:Banmen,
				mochigoma:MochigomaCollections,
				mvs:Vec<Move>, elapsed:Vec<u32>) -> CsaData {
		CsaData {
			version:None,
			kifu_info:None,
			initial_position:banmen,
			initial_mochigoma:mochigoma,
			moves:mvs,
			elapsed:elapsed,
			end_state:None,
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
}
#[derive(Debug)]
pub enum EndState {
	Toryo, // 投了
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