#![doc(html_root_url = "https://docs.rs/adb-dump/0.0.1")]
#![warn(clippy::pedantic)]
#![allow(clippy::missing_errors_doc)]

use std::{
	any::type_name,
	borrow::Borrow,
	convert::{TryFrom, TryInto},
	ffi::{OsStr, OsString},
	fmt::{Debug, Display, Formatter},
	io::{Error, ErrorKind},
	iter,
	ops::{Deref, Index, Range, RangeFrom, RangeTo},
	process::{Command, Output},
	time::{Duration, UNIX_EPOCH},
};

#[cfg(doctest)]
pub mod readme {
	doc_comment::doctest!("../README.md");
}

macro_rules! unix_mode_fn {
	($name:ident) => {
		pub fn $name(&self) -> bool {
			unix_mode::$name(self.0)
		}
	};
	($($name:ident,)*) => {
		$(unix_mode_fn!($name);)*
	};
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
	pub fn to_string(&self) -> String {
		unix_mode::to_string(self.0)
	}

	pub fn to_u32(&self) -> u32 {
		self.0
	}
}

impl Debug for UnixMode {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		Display::fmt(&self.to_string(), f)
	}
}

#[derive(Debug)]
pub struct LsEntry {
	mode: UnixMode,
	size: u32,
	epoch: Epoch,
	name: RawString,
}

#[derive(Debug)]
pub struct Epoch(u32);
impl Epoch {
	#[must_use]
	pub fn from_timestamp(secs: u32) -> Self {
		Self(secs)
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
struct AnError(&'static str);
impl Display for AnError {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		Display::fmt(self.0, f)
	}
}
impl std::error::Error for AnError {}

pub fn get_serialno() -> Result<SerialNumber, Error> {
	Ok(SerialNumber(
		single_line(RawStr::new(&scrape(
			"adb",
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
	let ls_out = RawString(scrape(
		"adb",
		[RawStr::new("-s"), serial_number, RawStr::new("ls"), path]
			.iter()
			.copied(),
	)?);
	let lines = ls_out
		.lines()
		.map(|mut l| {
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

	Ok(lines.into_iter())
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
						AnError("Wrong number of bytes in hex u32"),
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

fn scrape<'a>(
	command: &(impl AsRef<OsStr> + ?Sized),
	args: impl IntoIterator<Item = &'a RawStr>,
) -> Result<Vec<u8>, Error> {
	let output = Command::new("adb")
		.args(
			args.into_iter()
				.map(|s| OsString::try_from(s))
				.collect::<Result<Vec<_>, _>>()?,
		)
		.output()?;
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
pub struct RawPathBuf(Vec<u8>);

#[repr(transparent)]
pub struct RawPath(RawStr);
impl RawPath {
	pub fn new(data: &(impl AsRef<[u8]> + ?Sized)) -> &Self {
		let ptr = RawStr::new(data.as_ref()) as *const _ as *const RawPath;
		unsafe { &*ptr }
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

	pub fn as_dbg(&self) -> impl Debug + Sized + '_ {}
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

impl Debug for RawString {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		Debug::fmt(&String::from_utf8_lossy(&self.0), f)
	}
}
