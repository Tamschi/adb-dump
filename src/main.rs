#![warn(clippy::pedantic)]

use adb_dump::{LsEntry, ModeKind, RawPath, SerialNumber};
use chrono::{Datelike, NaiveDateTime, Timelike};
use std::{
	convert::TryFrom,
	fs::File,
	io::{Error, Write},
};
use zip::{write::FileOptions, CompressionMethod, DateTime, ZipWriter};

fn start_zip(zip_count: &mut usize) -> Result<ZipWriter<File>, Error> {
	*zip_count += 1;
	let file = std::fs::OpenOptions::new()
		.create_new(true)
		.write(true)
		.open(&format!("backup.{}.zip", zip_count))?;
	Ok(ZipWriter::new(file))
}

fn main() -> Result<(), Error> {
	let s_no = dbg!(adb_dump::get_serialno())?;

	let arg_path: &RawPath = "/data".into();
	let prefix = arg_path.directory().unwrap();

	let mut zip_count = 0;
	let mut cumulative_file_size = 0;
	let mut zip = start_zip(&mut zip_count)?;

	visit_dir(
		&mut zip,
		&mut zip_count,
		&mut cumulative_file_size,
		&s_no,
		prefix,
		arg_path,
	)?;

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

fn visit_dir(
	zip: &mut ZipWriter<File>,
	zip_count: &mut usize,
	cumulative_file_size: &mut usize,
	serial_number: &SerialNumber,
	archive_root: &RawPath,
	path: &RawPath,
) -> Result<(), Error> {
	println!("dir {:?}", &path);

	let ignore = &[
		"/BrowserMetrics", // LineageOS used to generate a very large (unbounded) amount of these files via web embed, and they're quite large too.
		"/HTTP Cache",
		"/com.google.android.googlequicksearchbox", // This caches A LOT of data and you probably don't want to keep it.
		"/.com.google.firebase.crashlytics-ndk",    // Twitter creates a huge amount of crash reports :(
		"/org.mozilla.firefox/cache",               // It's just a ton of pretty useless files.
		"/com.crashlytics.sdk.android.crashlytics-core", // Notify crashes a lot.
		"/lib/python2.7",                           // Ren'Py uses this.
		"/cache/image_manager_disk_cache", // Various programs use this, so while it's usually not huge, there might be a lot cumulatively.
		"/cache:memrise.offline.assets",   // Many files, and quite possibly large.
		"/org.mozilla.firefox_beta/cache",
		"/com.ecosia.android/cache",            // Another browser cache.
		"/com.duckduckgo.mobile.android/cache", // Quack.
		"/org.mozilla.fenix/cache",             // Firefox Nightly
		"/data/data/com.google.android.apps.photos/cache/glide_cache", // Google Photos thumbnail cache, probably. It can take ages to back this up.
	];
	for ignore in ignore {
		if path.to_string_panicky().ends_with(ignore) {
			zip.start_file(
				path.without_prefix(archive_root)
					.join("IGNORED")
					.to_string_panicky(),
				FileOptions::default(),
			)?;
			eprintln!("IGNORED");
			return Ok(());
		}
	}

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
						zip_count,
						cumulative_file_size,
						serial_number,
						archive_root,
						&path.join(entry.name.as_str()),
					)?
				}
			}
			file if file == ModeKind::File => visit_file(
				zip,
				zip_count,
				cumulative_file_size,
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

fn visit_file(
	zip: &mut ZipWriter<File>,
	zip_count: &mut usize,
	cumulative_file_size: &mut usize,
	serial_number: &SerialNumber,
	archive_root: &RawPath,
	path: &RawPath,
	entry: &LsEntry,
) -> Result<(), Error> {
	println!("file {:?}", &path);

	let file = adb_dump::pull(serial_number, path, entry.size)?;

	*cumulative_file_size += file.len();
	if *cumulative_file_size > 1_000_000_000 {
		*cumulative_file_size = file.len();

		zip.finish()?;
		*zip = start_zip(zip_count)?;
	}

	zip.start_file(
		path.without_prefix(archive_root).to_string_panicky(),
		FileOptions::default()
			.compression_method(CompressionMethod::STORE)
			.last_modified_time(convert_date_time(&entry.epoch.to_date_time()))
			.unix_permissions(entry.mode.permissions()),
	)?;
	zip.write_all(&file)?;
	zip.flush()?;
	Ok(())
}
