use std::collections::BTreeMap;

use aws_nitro_enclaves_nsm_api::api::Digest;

use super::*;

/// Mandatory field
pub(super) fn module_id(id: &String) -> Result<(), AttestError> {
	if id.len() < 1 {
		Err(AttestError::InvalidModuleId)
	} else {
		Ok(())
	}
}
/// Mandatory field
pub(super) fn pcrs(pcrs: &BTreeMap<usize, ByteBuf>) -> Result<(), AttestError> {
	let is_valid_pcr_count = pcrs.len() > 0 && pcrs.len() <= 32;

	let is_valid_index_and_len = pcrs.iter().all(|(idx, pcr)| {
		let is_valid_idx = *idx <= 32;
		let is_valid_pcr_len = [32, 48, 64].contains(&pcr.len());
		is_valid_idx && is_valid_pcr_len
	});

	if !is_valid_index_and_len || !is_valid_pcr_count {
		Err(AttestError::InvalidPcr)
	} else {
		Ok(())
	}
}
/// Mandatory field
pub(super) fn cabundle(cabundle: &Vec<ByteBuf>) -> Result<(), AttestError> {
	let is_valid_len = cabundle.len() > 0;
	let is_valid_entries =
		cabundle.iter().all(|cert| cert.len() >= 1 && cert.len() <= 1024);

	if !is_valid_len || !is_valid_entries {
		Err(AttestError::InvalidCABundle)
	} else {
		Ok(())
	}
}
/// Mandatory field
pub(super) fn digest(d: Digest) -> Result<(), AttestError> {
	if d != Digest::SHA384 {
		Err(AttestError::InvalidDigest)
	} else {
		Ok(())
	}
}
/// Mandatory field
pub(super) fn timestamp(t: u64) -> Result<(), AttestError> {
	if t == 0 {
		Err(AttestError::InvalidTimeStamp)
	} else {
		Ok(())
	}
}
/// Optional field
pub(super) fn public_key(pub_key: &Option<ByteBuf>) -> Result<(), AttestError> {
	if let Some(key) = pub_key {
		(key.len() >= 1 && key.len() <= 1024)
			.then(|| ())
			.ok_or(AttestError::InvalidPubKey)?
	}

	Ok(())
}
/// Optional field
pub(super) fn user_data(data: &Option<ByteBuf>) -> Result<(), AttestError> {
	bytes_512(data)
}
/// Optional field
pub(super) fn nonce(n: &Option<ByteBuf>) -> Result<(), AttestError> {
	bytes_512(n)
}

fn bytes_512(val: &Option<ByteBuf>) -> Result<(), AttestError> {
	if let Some(val) = val {
		(val.len() <= 512).then(|| ()).ok_or(AttestError::InvalidBytes)?
	}

	Ok(())
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn user_data_works() {
		assert!(user_data(&None).is_ok());
		assert!(user_data(&Some(ByteBuf::new())).is_ok());
		assert!(user_data(&Some(ByteBuf::from(
			(0..513).map(|_| 42u8).collect::<Vec<_>>()
		)))
		.is_err());
	}

	#[test]
	fn nonce_works() {
		assert!(nonce(&None).is_ok());
		assert!(nonce(&Some(ByteBuf::new())).is_ok());
		assert!(nonce(&Some(ByteBuf::from(
			(0..513).map(|_| 42u8).collect::<Vec<_>>()
		)))
		.is_err());
	}

	#[test]
	fn public_key_works() {
		assert!(public_key(&None).is_ok());
		assert!(public_key(&Some(ByteBuf::new())).is_err());
		assert!(public_key(&Some(ByteBuf::from(vec![1u8]))).is_ok());
		assert!(public_key(&Some(ByteBuf::from(
			(0..1025).map(|_| 42u8).collect::<Vec<_>>()
		)))
		.is_err());
	}

	#[test]
	fn timestamp_works() {
		assert!(timestamp(0).is_err());
		assert!(timestamp(1).is_ok());
	}

	#[test]
	fn digest_works() {
		assert!(digest(Digest::SHA256).is_err());
		assert!(digest(Digest::SHA384).is_ok());
	}

	#[test]
	fn cabundle_works() {
		let valid_cert = ByteBuf::from(vec![42]);
		assert!(cabundle(&vec![valid_cert]).is_ok());

		assert!(cabundle(&vec![]).is_err());

		let short_cert = ByteBuf::new();
		assert!(cabundle(&vec![short_cert]).is_err());

		let long_cert = ByteBuf::from((0..1025).map(|_| 3).collect::<Vec<_>>());
		assert!(cabundle(&vec![long_cert]).is_err());
	}

	#[test]
	fn pcrs_works() {
		let pcr32 = ByteBuf::from((0..32).map(|_| 3).collect::<Vec<_>>());
		let pcr48 = ByteBuf::from((0..48).map(|_| 3).collect::<Vec<_>>());
		let pcr64 = ByteBuf::from((0..64).map(|_| 3).collect::<Vec<_>>());

		let inner: [(usize, ByteBuf); 33] = (0..33)
			.map(|i| (i, pcr32.clone()))
			.collect::<Vec<_>>()
			.try_into()
			.unwrap();
		let too_many_pcrs = BTreeMap::from(inner);
		assert!(pcrs(&too_many_pcrs).is_err());

		let too_few_pcrs = BTreeMap::new();
		assert!(pcrs(&too_few_pcrs).is_err());

		// Invalid PCR index
		assert!(pcrs(&BTreeMap::from([(33, pcr32.clone())])).is_err());

		// Valid
		assert!(pcrs(&BTreeMap::from([
			(0, pcr48.clone()),
			(32, pcr32.clone()),
			(5, pcr64.clone())
		]))
		.is_ok());
	}
}
