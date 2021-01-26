#![warn(clippy::pedantic)]

use adb_dump::{LsEntry, ModeKind, RawPath, SerialNumber};
use chrono::{Datelike, NaiveDateTime, Timelike};
use std::{
	convert::TryFrom,
	fs::File,
	io::{Error, Seek, Write},
};
use zip::{write::FileOptions, CompressionMethod, DateTime, ZipWriter};

fn main() -> Result<(), Error> {
	let s_no = dbg!(adb_dump::get_serialno())?;

	let arg_path: &RawPath = "/data".into();
	let prefix = arg_path.directory().unwrap();

	let file = std::fs::OpenOptions::new()
		.create_new(true)
		.write(true)
		.open("backup.zip")?;
	let mut zip = ZipWriter::new(file);

	visit_dir(&mut zip, &s_no, prefix, arg_path)?;

	zip.finish()?;

	Ok(())
}

fn convert_date_time(date_time: &NaiveDateTime) -> DateTime {
	let time = date_time.time();
	DateTime::from_date_and_time(
		u16::try_from(date_time.year()).unwrap(),
		u8::try_from(date_time.month()).unwrap(),
		u8::try_from(date_time.day()).unwrap(),
		u8::try_from(time.hour()).unwrap(),
		u8::try_from(time.minute()).unwrap(),
		u8::try_from(time.second()).unwrap(),
	)
	.unwrap_or_else(|()| {
		let default = DateTime::default();
		eprintln! {"Ignoring date: {:?}, using {:?} instead", date_time,default};
		default
	})
}

fn visit_dir<W: Write + Seek>(
	zip: &mut ZipWriter<W>,
	serial_number: &SerialNumber,
	archive_root: &RawPath,
	path: &RawPath,
) -> Result<(), Error> {
	println!("dir {:?}", &path);

	for entry in adb_dump::ls(serial_number, path)? {
		match entry.mode.kind() {
			dir if dir == ModeKind::Dir => {
				if entry.name != "." && entry.name != ".." {
					let timestamp = convert_date_time(&entry.epoch.to_date_time());
					zip.add_directory(
						path.without_prefix(archive_root)
							.join(entry.name.as_str())
							.to_string_panicky(),
						FileOptions::default()
							.compression_method(CompressionMethod::Stored)
							.last_modified_time(timestamp)
							.unix_permissions(entry.mode.permissions()),
					)?;
					//TODO: Add dir!
					visit_dir(
						zip,
						serial_number,
						archive_root,
						&path.join(entry.name.as_str()),
					)?
				}
			}
			file if file == ModeKind::File => visit_file(
				zip,
				serial_number,
				archive_root,
				&path.join(entry.name.as_str()),
				&entry,
			)?,
			// other => todo!("{:?}", other),
			other => {
				eprintln!("{:?}", (other, entry.name));
			}
		}
	}

	Ok(())
}

fn visit_file<W: Write + Seek>(
	zip: &mut ZipWriter<W>,
	serial_number: &SerialNumber,
	archive_root: &RawPath,
	path: &RawPath,
	entry: &LsEntry,
) -> Result<(), Error> {
	println!("file {:?}", &path);

	zip.start_file(
		path.without_prefix(archive_root).to_string_panicky(),
		FileOptions::default()
			.compression_method(CompressionMethod::DEFLATE)
			.last_modified_time(convert_date_time(&entry.epoch.to_date_time()))
			.unix_permissions(entry.mode.permissions()),
	)?;
	zip.write_all(&adb_dump::pull(serial_number, path, entry.size)?)?;
	zip.flush()?;
	Ok(())
}
