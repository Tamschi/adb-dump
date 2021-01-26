#![warn(clippy::pedantic)]

use std::io::Error;

fn main() -> Result<(), Error> {
	let s_no = dbg!(adb_dump::get_serialno())?;
	dbg!(adb_dump::ls(&s_no, "/")?.collect::<Vec<_>>());
	Ok(())
}
