use cosmwasm_std::Timestamp;

// TODO: Make this executable in a const context when possible, related issues:
// https://github.com/rust-lang/rust/issues/60551 https://github.com/rust-lang/rust/issues/76560
pub trait TimestampExtentions {
	/// Creates a timestamp from milliseconds since epoch
	fn from_millis(milliseconds_since_epoch: u64) -> Self;
	/// Returns milliseconds since epoch (truncate nanoseconds)
	fn millis(&self) -> u64;
}

impl TimestampExtentions for Timestamp {
	#[inline]
	fn from_millis(milliseconds_since_epoch: u64) -> Self {
		Timestamp::from_nanos(milliseconds_since_epoch * 1_000_000)
	}
	
	#[inline]
	fn millis(&self) -> u64 {
		self.nanos() / 1_000_000
	}
}
