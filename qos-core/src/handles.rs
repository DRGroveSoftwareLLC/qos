//! Logic for accessing read only QOS state.

use std::{fs, os::unix::fs::PermissionsExt, path::Path};

use borsh::{BorshDeserialize, BorshSerialize};
use qos_crypto::RsaPair;

use crate::protocol::{services::boot::ManifestEnvelope, ProtocolError};

/// Handles for read only state accessible to all of QOS.
///
/// All data here should be "put" once at some point in the boot flow. Once
/// "put", it can only be read.
#[derive(Debug, Clone)]
pub struct Handles {
	/// Path to the file containing the PEM encoded Ephemeral Key.
	ephemeral: String,
	/// Path to the file containing the PEM encoded Quorum Key.
	quorum: String,
	/// Path to the file containing the Borsh encoded [`ManifestEnvelope`].
	manifest: String,
	/// Path to the file containing the pivot.
	pivot: String,
}

impl Handles {
	/// Create a new instance of [`Self`].
	#[must_use]
	pub fn new(
		ephemeral: String,
		quorum: String,
		manifest: String,
		pivot: String,
	) -> Self {
		Self { ephemeral, quorum, manifest, pivot }
	}

	/// Get the path to the Ephemeral Key.
	#[must_use]
	pub fn ephemeral_key_path(&self) -> String {
		self.ephemeral.clone()
	}

	/// Get the Ephemeral Key pair.
	///
	/// # Errors
	///
	/// Errors if the Ephemeral Key has not been put.
	pub fn get_ephemeral_key(&self) -> Result<RsaPair, ProtocolError> {
		let pair = RsaPair::from_pem_file(&self.ephemeral)
			.map_err(|_| ProtocolError::FailedToGetEphemeralKey)?;
		Ok(pair)
	}

	/// Put the Ephemeral Key pair.
	///
	/// # Errors
	///
	/// Errors if the Ephemeral Key has already been put.
	pub fn put_ephemeral_key(
		&self,
		pair: &RsaPair,
	) -> Result<(), ProtocolError> {
		Self::write_as_read_only(
			&self.ephemeral,
			&pair.private_key_to_pem()?,
			ProtocolError::FailedToPutEphemeralKey,
		)
	}

	/// Put the Quorum Key pair.
	///
	/// # Errors
	///
	/// Errors if the Quorum Key has already been put.
	pub fn put_quorum_key(&self, pair: &RsaPair) -> Result<(), ProtocolError> {
		Self::write_as_read_only(
			&self.quorum,
			&pair.private_key_to_pem()?,
			ProtocolError::FailedToPutManifestEnvelope,
		)
	}

	/// Returns true if the Quorum Key file exists.
	#[must_use]
	pub fn quorum_key_exists(&self) -> bool {
		Path::new(&self.quorum).exists()
	}

	/// Get the Manifest.
	///
	/// # Errors
	///
	/// Errors if the Manifest has not been put.
	pub fn get_manifest_envelope(
		&self,
	) -> Result<ManifestEnvelope, ProtocolError> {
		let contents = fs::read(&self.manifest)
			.map_err(|_| ProtocolError::FailedToGetManifestEnvelope)?;
		let manifest = ManifestEnvelope::try_from_slice(&contents)
			.map_err(|_| ProtocolError::FailedToGetManifestEnvelope)?;
		Ok(manifest)
	}

	/// Put the Manifest.
	///
	/// # Errors
	///
	/// Errors if the Manifest has already been put.
	pub fn put_manifest_envelope(
		&self,
		manifest_envelope: &ManifestEnvelope,
	) -> Result<(), ProtocolError> {
		Self::write_as_read_only(
			&self.manifest,
			&manifest_envelope.try_to_vec()?,
			ProtocolError::FailedToPutManifestEnvelope,
		)
	}

	/// Returns true if the Manifest file exists.
	#[must_use]
	pub fn manifest_envelope_exists(&self) -> bool {
		Path::new(&self.manifest).exists()
	}

	/// Get the path to the Pivot binary.
	#[must_use]
	pub fn pivot_path(&self) -> String {
		self.pivot.clone()
	}

	/// Put the Pivot binary, ensuring it is an executable.
	pub fn put_pivot(&self, pivot: &[u8]) -> Result<(), ProtocolError> {
		if Path::new(&self.pivot).exists() {
			Err(ProtocolError::CannotModifyPostPivotStatic)?;
		}

		if let Some(parent) = Path::new(&self.pivot).parent() {
			if !parent.exists() {
				fs::create_dir_all(parent)
					.map_err(|_| ProtocolError::FailedToPutPivot)?;
			}
		}

		fs::write(&self.pivot, pivot)
			.map_err(|_| ProtocolError::FailedToPutPivot)?;
		fs::set_permissions(
			&self.pivot,
			std::fs::Permissions::from_mode(0o111),
		)
		.map_err(|_| ProtocolError::FailedToPutPivot)?;
		Ok(())
	}

	/// Returns true if the Pivot file exists.
	#[must_use]
	pub fn pivot_exists(&self) -> bool {
		Path::new(&self.pivot).exists()
	}

	/// Helper function for ready only writes.
	fn write_as_read_only<P: AsRef<Path>>(
		path: P,
		buf: &[u8],
		err: ProtocolError,
	) -> Result<(), ProtocolError> {
		if path.as_ref().exists() {
			Err(ProtocolError::CannotModifyPostPivotStatic)?;
		}

		if let Some(parent) = path.as_ref().parent() {
			if !parent.exists() {
				fs::create_dir_all(parent).map_err(|_| err.clone())?;
			}
		}

		fs::write(&path, buf).map_err(|_| err.clone())?;
		fs::set_permissions(&path, fs::Permissions::from_mode(0o444))
			.map_err(|_| err)?;

		Ok(())
	}
}

// TODO unit tests <https://github.com/tkhq/qos/issues/78/>
