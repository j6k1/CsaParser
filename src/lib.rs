extern crate usiagent;

pub trait CsaStream {
	fn next() -> Option<String>;
	fn read_real_line(l:String) -> Vec<String> {
		if l.starts_with('\'') {
			vec![String::from(&l.as_str()[1..])]
		} else {
			l.split(",").collect::<Vec<&str>>().into_iter().map(|s| s.to_string()).collect::<Vec<String>>()
		}
	}
}
