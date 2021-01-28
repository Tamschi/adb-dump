#![doc(html_root_url = "https://docs.rs/adb-dump/0.0.1")]
#![warn(clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]

use chrono::NaiveDateTime;
use enumflags2::BitFlags;
use std::{
	any::type_name,
	borrow::{Borrow, Cow},
	convert::{TryFrom, TryInto},
	ffi::{OsStr, OsString},
	fmt::{Debug, Display, Formatter},
	io::{Error, ErrorKind},
	iter,
	ops::{Add, AddAssign, Deref, Index, Range, RangeFrom, RangeInclusive, RangeTo},
	path::PathBuf,
	process::{Command, Output},
	time::{Duration, SystemTime, UNIX_EPOCH},
};

#[cfg(doctest)]
pub mod readme {
	doc_comment::doctest!("../README.md");
}

macro_rules! unix_mode_fn {
	($name:ident) => {
		#[must_use]
		pub fn $name(&self) -> bool {
			unix_mode::$name(self.0)
		}
	};
	($($name:ident,)*) => {
		$(unix_mode_fn!($name);)*
	};
}

#[derive(BitFlags, Clone, Copy, Debug, PartialEq)]
pub enum ModeKind {
	BlockDevice = 1 << 0,
	CharDevice = 1 << 1,
	Dir = 1 << 2,
	Fifo = 1 << 3,
	File = 1 << 4,
	Socket = 1 << 5,
	Symlink = 1 << 6,
}

pub struct UnixMode(u32);
impl UnixMode {
	pub fn new(value: u32) -> Self {
		Self(value)
	}

	unix_mode_fn! {
		is_block_device,
		is_char_device,
		is_dir,
		is_fifo,
		is_file,
		is_socket,
		is_symlink,
	}

	#[must_use]
	pub fn to_u32(&self) -> u32 {
		self.0
	}

	#[must_use]
	pub fn kind(&self) -> BitFlags<ModeKind> {
		let mut result = BitFlags::empty();
		if self.is_block_device() {
			result |= ModeKind::BlockDevice
		}
		if self.is_char_device() {
			result |= ModeKind::CharDevice
		}
		if self.is_dir() {
			result |= ModeKind::Dir
		}
		if self.is_fifo() {
			result |= ModeKind::Fifo
		}
		if self.is_file() {
			result |= ModeKind::File
		}
		if self.is_socket() {
			result |= ModeKind::Socket
		}
		if self.is_symlink() {
			result |= ModeKind::Symlink
		}
		result
	}
}
impl ToString for UnixMode {
	#[must_use]
	fn to_string(&self) -> String {
		unix_mode::to_string(self.0)
	}
}

impl Debug for UnixMode {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		Display::fmt(&self.to_string(), f)
	}
}

#[derive(Debug)]
pub struct LsEntry {
	pub mode: UnixMode,
	pub size: u32,
	pub epoch: Epoch,
	pub name: RawString,
}

impl UnixMode {
	#[must_use]
	pub fn permissions(&self) -> u32 {
		self.0 & 0o_777
	}
}

#[derive(Debug)]
pub struct Epoch(u32);
impl Epoch {
	#[must_use]
	pub fn from_timestamp(secs: u32) -> Self {
		Self(secs)
	}
}

impl RawPath {
	#[must_use]
	pub fn directory(&self) -> Option<&Self> {
		self.iter().rposition(|b| *b == b'/').map(|p| &self[0..=p])
	}

	#[must_use]
	pub fn without_prefix(&self, prefix: &Self) -> &Self {
		assert!(self.starts_with(&prefix.0 .0));
		&self[prefix.len()..]
	}
}

impl<I> Index<I> for RawPath
where
	RawStr: Index<I, Output = RawStr>,
{
	type Output = RawPath;

	fn index(&self, index: I) -> &Self::Output {
		(&self.0[index]).into()
	}
}

impl Epoch {
	#[must_use]
	pub fn to_date_time(&self) -> NaiveDateTime {
		NaiveDateTime::from_timestamp(i64::from(self.0), 0)
	}
}

impl RawPath {
	#[must_use]
	pub fn to_string_panicky(&self) -> String {
		self.0.to_string_panicky()
	}
}

impl RawStr {
	#[must_use]
	pub fn to_string_panicky(&self) -> String {
		std::string::String::from_utf8(self.to_vec()).unwrap()
	}
}

impl PartialEq<&str> for RawStr {
	fn eq(&self, other: &&str) -> bool {
		self.0 == *other.as_bytes()
	}
}

impl<T: ?Sized> PartialEq<T> for RawString
where
	RawStr: PartialEq<T>,
{
	fn eq(&self, other: &T) -> bool {
		self.as_str() == other
	}
}

impl RawString {
	pub fn as_str(&self) -> &RawStr {
		RawStr::new(&self.0)
	}
}

impl<'a> From<&'a RawStr> for &'a RawPath {
	fn from(str: &'a RawStr) -> Self {
		RawPath::new(&str.0)
	}
}

impl RawPath {
	pub fn join<'a>(&self, other: impl Into<&'a RawPath>) -> RawPathBuf {
		self.join_impl(other.into())
	}

	pub fn join_impl(&self, other: &RawPath) -> RawPathBuf {
		let mut slash = 1;
		if self.ends_with(&[b'/']) {
			slash -= 1
		}
		if other.ends_with(&[b'/']) {
			slash -= 1
		}
		let mut result = Vec::new();
		result.extend(&self.0 .0);
		while slash > 0 {
			result.push(b'/');
			slash -= 1;
		}
		while slash < 0 {
			assert_eq!(result.pop(), Some(b'/'));
			slash += 1;
		}
		assert_eq!(slash, 0);
		result.extend(&other.0 .0);
		RawPathBuf(RawString(result))
	}
}

impl Borrow<RawPath> for RawPathBuf {
	fn borrow(&self) -> &RawPath {
		RawPath::new(&self.0 .0)
	}
}

impl ToOwned for RawPath {
	type Owned = RawPathBuf;

	fn to_owned(&self) -> Self::Owned {
		RawPathBuf(self.0.to_owned())
	}
}

pub struct RawString(Vec<u8>);
impl Deref for RawString {
	type Target = RawStr;

	fn deref(&self) -> &Self::Target {
		RawStr::new(&self.0)
	}
}
pub struct SerialNumber(RawString);
impl Debug for SerialNumber {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		Debug::fmt(&String::from_utf8_lossy(&self.0 .0), f)
	}
}

#[derive(Debug)]
struct AnError<T: Debug + Display>(T);
impl<T: Debug + Display> Display for AnError<T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		Display::fmt(&self.0, f)
	}
}
impl<T: Debug + Display> std::error::Error for AnError<T> {}

pub fn get_serialno() -> Result<SerialNumber, Error> {
	Ok(SerialNumber(
		single_line(RawStr::new(&scrape_adb(
			[RawStr::new("get-serialno")].iter().copied(),
		)?))?
		.to_owned(),
	))
}

fn single_line(str: &RawStr) -> Result<&RawStr, Error> {
	let mut lines = str.lines();
	let line = lines
		.next()
		.ok_or_else(|| Error::new(ErrorKind::UnexpectedEof, AnError("No line found")))?;
	if lines.next().is_some() {
		return Err(Error::new(
			ErrorKind::InvalidData,
			AnError("Unexpected extra line found"),
		));
	}
	if line.len() == 0 {
		return Err(Error::new(
			ErrorKind::InvalidData,
			"No serial number found in output",
		));
	}
	Ok(line)
}

impl ToOwned for RawStr {
	type Owned = RawString;

	fn to_owned(&self) -> Self::Owned {
		RawString(self.0.to_vec())
	}
}

impl Borrow<RawStr> for RawString {
	fn borrow(&self) -> &RawStr {
		RawStr::new(&self.0)
	}
}

pub fn ls(
	serial_number: &SerialNumber,
	path: &(impl AsRef<RawPath> + ?Sized),
) -> Result<impl Iterator<Item = LsEntry>, Error> {
	ls_impl(serial_number, path.as_ref())
}

impl Deref for SerialNumber {
	type Target = RawString;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

pub fn ls_impl(
	serial_number: &SerialNumber,
	path: &RawPath,
) -> Result<impl Iterator<Item = LsEntry>, Error> {
	#![allow(clippy::items_after_statements)]

	let ls_out = RawString(scrape_adb(
		[RawStr::new("-s"), serial_number, RawStr::new("ls"), path]
			.iter()
			.copied(),
	)?);

	let mut lines = Vec::new();
	fn contains_another_hex_field(line: &mut &RawStr) -> bool {
		line.split_take(b' ').map(|slice| {
			slice.len() == 8
				&& slice
					.iter()
					.all(|b| (b'0'..=b'9').contains(b) || (b'a'..=b'f').contains(b))
		}) == Some(true)
	};
	for line in ls_out.lines() {
		let mut l = line;
		if contains_another_hex_field(&mut l)
			&& contains_another_hex_field(&mut l)
			&& contains_another_hex_field(&mut l)
		{
			// This is *probably* the start of a new line.
			lines.push(line.to_owned())
		} else if let Some(previous) = lines.last_mut() {
			previous.0.push(b'\n');
			*previous += line;
		} else {
			return Err(Error::new(
				ErrorKind::InvalidData,
				AnError(format!(
					"Could not make sense of `adb ls` output for path {:?}",
					path
				)),
			));
		}
	}

	let entries = lines
		.into_iter()
		.map(|l| {
			let mut l = l.as_str();
			Result::<_, Error>::Ok(LsEntry {
				mode: UnixMode(
					l.split_take(b' ')
						.ok_or_else(|| {
							Error::new(ErrorKind::InvalidData, AnError("No mode found"))
						})?
						.try_into()?,
				),
				size: l
					.split_take(b' ')
					.ok_or_else(|| Error::new(ErrorKind::InvalidData, AnError("No mode found")))?
					.try_into()?,
				epoch: Epoch(
					l.split_take(b' ')
						.ok_or_else(|| {
							Error::new(ErrorKind::InvalidData, AnError("No mode found"))
						})?
						.try_into()?,
				),
				name: l.to_owned(),
			})
		})
		.collect::<Result<Vec<_>, _>>()?;

	Ok(entries.into_iter())
}

impl AddAssign<&RawStr> for RawString {
	fn add_assign(&mut self, rhs: &RawStr) {
		self.0.extend(rhs.iter())
	}
}

impl TryFrom<&RawStr> for u32 {
	type Error = Error;

	fn try_from(value: &RawStr) -> Result<Self, Self::Error> {
		let str =
			std::str::from_utf8(value).map_err(|err| Error::new(ErrorKind::InvalidData, err))?;
		Ok(u32::from_be_bytes(
			hex::decode(str)
				.map_err(|err| Error::new(ErrorKind::InvalidData, err))?
				.try_into()
				.map_err(|err| {
					Error::new(
						ErrorKind::InvalidData,
						AnError(format!("Wrong number of bytes in hex u32: {:?}", err)),
					)
				})?,
		))
	}
}

impl Deref for RawStr {
	type Target = [u8];

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}

fn scrape_adb<'a>(args: impl IntoIterator<Item = &'a RawStr>) -> Result<Vec<u8>, Error> {
	let args = args
		.into_iter()
		.map(OsString::try_from)
		.collect::<Result<Vec<OsString>, _>>()?;
	let output = Command::new("adb").args(args.iter()).output()?;
	if output.status.success() {
		Ok(output.stdout)
	} else {
		Err(Error::new(ErrorKind::Other, ExitError(output)))
	}
}

pub struct ExitError(Output);
impl std::error::Error for ExitError {}
impl Debug for ExitError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		write!(f, "ExitError ")?;
		f.debug_struct(type_name::<Self>())
			.field("status", &self.0.status)
			.field("stdout", &RawStr::new(&self.0.stdout).as_dbg())
			.field("stderr", &RawStr::new(&self.0.stderr).as_dbg())
			.finish()
	}
}
impl Display for ExitError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		Debug::fmt(self, f)
	}
}

#[repr(transparent)]
pub struct RawPathBuf(RawString);

impl Debug for RawPathBuf {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		<RawPath as Debug>::fmt(&self, f)
	}
}

impl Deref for RawPathBuf {
	type Target = RawPath;

	fn deref(&self) -> &Self::Target {
		RawPath::new(&self.0 .0)
	}
}

impl From<&str> for RawPathBuf {
	fn from(str: &str) -> Self {
		Self(str.into())
	}
}

impl From<&str> for RawString {
	fn from(str: &str) -> Self {
		Self(str.into())
	}
}

#[derive(Debug)]
#[repr(transparent)]
pub struct RawPath(RawStr);
impl RawPath {
	pub fn new(data: &(impl AsRef<[u8]> + ?Sized)) -> &Self {
		let ptr = RawStr::new(data.as_ref()) as *const _ as *const RawPath;
		unsafe { &*ptr }
	}
}

impl AsRef<RawPath> for RawPath {
	fn as_ref(&self) -> &RawPath {
		self
	}
}

impl<'a> From<&'a str> for &'a RawPath {
	fn from(str: &'a str) -> Self {
		RawPath::new(str.as_bytes())
	}
}

impl Deref for RawPath {
	type Target = RawStr;

	fn deref(&self) -> &Self::Target {
		let ptr = self as *const _ as *const RawStr;
		unsafe { &*ptr }
	}
}

impl AsRef<RawPath> for str {
	fn as_ref(&self) -> &RawPath {
		RawPath::new(self.as_bytes())
	}
}

#[repr(transparent)]
pub struct RawStr([u8]);
impl RawStr {
	pub fn new(data: &(impl AsRef<[u8]> + ?Sized)) -> &Self {
		let ptr = data.as_ref() as *const _ as *const RawStr;
		unsafe { &*ptr }
	}

	pub fn len(&self) -> usize {
		self.0.len()
	}

	#[must_use]
	pub fn as_dbg(&self) -> impl Debug + Sized + '_ {
		self
	}
}
impl Debug for RawStr {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		Debug::fmt(&String::from_utf8_lossy(&self.0), f)
	}
}

struct Dbg<'a, T: Debug>(&'a T);
impl<'a, T: Debug> Debug for Dbg<'a, T> {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		self.0.fmt(f)
	}
}

impl AsRef<RawStr> for RawStr {
	fn as_ref(&self) -> &RawStr {
		self
	}
}
impl TryFrom<&RawStr> for OsString {
	type Error = Error;

	fn try_from(value: &RawStr) -> Result<Self, Self::Error> {
		match std::str::from_utf8(&value.0) {
			Ok(str) => Ok(str.into()),
			Err(err) => Err(Error::new(ErrorKind::InvalidData, err)),
		}
	}
}

impl RawStr {
	pub fn lines(&self) -> impl Iterator<Item = &'_ Self> {
		let mut prev = 0;
		self.0
			.iter()
			.copied()
			.enumerate()
			.filter_map(move |(i, b)| {
				if b == b'\r' && self.0[i + 1] == b'\n' {
					let start = prev;
					prev = i + 2;
					Some(&self[start..i])
				} else {
					None
				}
			})
	}

	pub fn split_take(self: &mut &Self, b: u8) -> Option<&RawStr> {
		match self.0.iter().position(|x| *x == b) {
			Some(i) => {
				let result = &self[..i];
				*self = &self[i + 1..];
				Some(result)
			}
			None => None,
		}
	}
}

impl Index<Range<usize>> for RawStr {
	type Output = RawStr;

	fn index(&self, index: Range<usize>) -> &Self::Output {
		RawStr::new(&self.0[index])
	}
}

impl Index<RangeTo<usize>> for RawStr {
	type Output = RawStr;

	fn index(&self, index: RangeTo<usize>) -> &Self::Output {
		RawStr::new(&self.0[index])
	}
}

impl Index<RangeFrom<usize>> for RawStr {
	type Output = RawStr;

	fn index(&self, index: RangeFrom<usize>) -> &Self::Output {
		RawStr::new(&self.0[index])
	}
}

impl Index<RangeInclusive<usize>> for RawStr {
	type Output = RawStr;

	fn index(&self, index: RangeInclusive<usize>) -> &Self::Output {
		RawStr::new(&self.0[index])
	}
}

impl Debug for RawString {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		Debug::fmt(&String::from_utf8_lossy(&self.0), f)
	}
}

pub fn pull(
	serial_number: &SerialNumber,
	path: &(impl AsRef<RawPath> + ?Sized),
	expected_size: u32,
) -> Result<Vec<u8>, Error> {
	pull_impl(serial_number, path.as_ref(), expected_size)
}

pub fn pull_impl(
	serial_number: &SerialNumber,
	path: &RawPath,
	expected_size: u32,
) -> Result<Vec<u8>, Error> {
	// let _it_works = scrape(
	// 	"adb",
	// 	[
	// 		RawStr::new("-s"),
	// 		serial_number,
	// 		RawStr::new("shell"),
	// 		RawStr::new("-n"),
	// 		RawStr::new("-T"),
	// 		RawStr::new("cat"),
	// 		RawStr::new(
	// 			shell_escape::escape(Cow::Owned(path.to_string_panicky()))
	// 				.replace('\'', "\\'")
	// 				.as_bytes(),
	// 		),
	// 	]
	// 	.iter()
	// 	.copied(),
	// )?;

	let file = scrape_adb(
		[
			RawStr::new("-s"),
			serial_number,
			RawStr::new("exec-out"),
			RawStr::new("cat"),
			// Must not be escaped!
			path,
		]
		.iter()
		.copied(),
	)?;

	if file.len() == usize::try_from(expected_size).unwrap() {
		Ok(file)
	} else {
		Err(Error::new(
			ErrorKind::InvalidData,
			AnError(format!(
				"Error pulling {:?}: Exprected {} bytes, got {}",
				path,
				expected_size,
				file.len()
			)),
		))
	}
}
