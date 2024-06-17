use cosmwasm_std::{DivideByZeroError, Uint128, Uint256, Uint512, Uint64};

pub trait UintMathExtensions: Sized {
	fn checked_div_ceil_int(self, other: Self) -> Result<Self, DivideByZeroError>;
	fn div_ceil_int(self, other: Self) -> Self;
}

macro_rules! impl_math_ext_dependencies_native {
	($cosm_type:ty, $native_type:ty) => {
		impl UintMathExtensions for $cosm_type {
			#[inline]
			fn checked_div_ceil_int(self, other: Self) -> Result<Self, DivideByZeroError> {
				if other.is_zero() {
					Err(DivideByZeroError::new(self))
				} else {
					Ok(Self::div_ceil_int(self, other))
				}
			}
			#[inline]
			fn div_ceil_int(self, other: Self) -> Self {
				<$cosm_type>::from(<$native_type>::from(self).div_ceil(<$native_type>::from(other)))
			}
		}
	};
}
impl_math_ext_dependencies_native!(Uint64, u64);
impl_math_ext_dependencies_native!(Uint128, u128);

macro_rules! impl_math_ext_dependencies_bnum {
	($cosm_type:ty, $bnum_type:ty) => {
		impl UintMathExtensions for $cosm_type {
			#[inline]
			fn checked_div_ceil_int(self, other: Self) -> Result<Self, DivideByZeroError> {
				if other.is_zero() {
					Err(DivideByZeroError::new(self))
				} else {
					Ok(Self::div_ceil_int(self, other))
				}
			}
			#[inline]
			fn div_ceil_int(self, other: Self) -> Self {
				// Uint256 doesn't let us access its inner U256, so we're doing this hack for now.
				<$cosm_type>::from_le_bytes(
					bytemuck::cast(
						*<$bnum_type>::from_le_slice(
							&self.to_le_bytes()
						).unwrap().div_ceil(
							<$bnum_type>::from_le_slice(
								&other.to_le_bytes()
							).unwrap()
						).digits()
					)
				)
			}
		}
	};
}
impl_math_ext_dependencies_bnum!(Uint256, bnum::types::U256);
impl_math_ext_dependencies_bnum!(Uint512, bnum::types::U512);

#[cfg(test)]
mod tests {
	use super::*;
	#[test]
	fn div_ceil_int() {
		let a = Uint64::from(1000u64);
		let b = Uint64::from(3u64);
		assert_eq!(a.div_ceil_int(b), Uint64::from(334u64));
		assert_eq!(a.checked_div_ceil_int(b), Ok(Uint64::from(334u64)));
		assert!(a.checked_div_ceil_int(Uint64::zero()).is_err());

		let a = Uint128::from(1000u128);
		let b = Uint128::from(3u128);
		assert_eq!(a.div_ceil_int(b), Uint128::from(334u128));
		assert_eq!(a.checked_div_ceil_int(b), Ok(Uint128::from(334u128)));
		assert!(a.checked_div_ceil_int(Uint128::zero()).is_err());

		let a = Uint256::from(1000u128);
		let b = Uint256::from(3u128);
		assert_eq!(a.div_ceil_int(b), Uint256::from(334u128));
		assert_eq!(a.checked_div_ceil_int(b), Ok(Uint256::from(334u128)));
		assert!(a.checked_div_ceil_int(Uint256::zero()).is_err());

		let a = Uint512::from(1000u128);
		let b = Uint512::from(3u128);
		assert_eq!(a.div_ceil_int(b), Uint512::from(334u128));
		assert_eq!(a.checked_div_ceil_int(b), Ok(Uint512::from(334u128)));
		assert!(a.checked_div_ceil_int(Uint512::zero()).is_err());
	}
}
