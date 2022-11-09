//! Abstractions for encryption.

use aes_gcm::{
	aead::{Aead, KeyInit, Payload},
	Aes256Gcm, Nonce,
};
use borsh::{BorshDeserialize, BorshSerialize};
use hmac::{Hmac, Mac};
use p256::{
	ecdh::diffie_hellman, elliptic_curve::sec1::ToEncodedPoint, PublicKey,
	SecretKey,
};
use rand_core::OsRng;
use sha2::Sha512;
use zeroize::ZeroizeOnDrop;

use crate::P256Error;

const AES256_KEY_LEN: usize = 32;
const BITS_96_AS_BYTES: usize = 12;
use crate::PUB_KEY_LEN_UNCOMPRESSED;

type HmacSha512 = Hmac<Sha512>;

/// Envelope for serializing an encrypted message with it's context.
#[derive(Debug, borsh::BorshSerialize, borsh::BorshDeserialize)]
struct Envelope {
	/// Nonce used as an input to the cipher.
	nonce: [u8; BITS_96_AS_BYTES],
	/// Public key as sec1 encoded point with no compression
	ephemeral_sender_public: [u8; PUB_KEY_LEN_UNCOMPRESSED],
	/// The data encrypted with an AES 256 GCM cipher.
	encrypted_message: Vec<u8>,
}

/// P256 key pair
#[derive(ZeroizeOnDrop)]
#[cfg_attr(any(feature = "mock", test), derive(Clone, PartialEq))]
pub struct P256EncryptPair {
	private: SecretKey,
}

impl P256EncryptPair {
	/// Generate a new private key using the OS randomness source.
	#[must_use]
	pub fn generate() -> Self {
		Self { private: SecretKey::random(&mut OsRng) }
	}

	/// Decrypt a message encoded to this pair's public key.
	pub fn decrypt(
		&self,
		serialized_envelope: &[u8],
	) -> Result<Vec<u8>, P256Error> {
		let Envelope {
			nonce,
			ephemeral_sender_public: ephemeral_sender_public_bytes,
			encrypted_message,
		} = Envelope::try_from_slice(serialized_envelope)
			.map_err(|_| P256Error::FailedToDeserializeEnvelope)?;

		let nonce = Nonce::from_slice(&nonce);
		let ephemeral_sender_public =
			PublicKey::from_sec1_bytes(&ephemeral_sender_public_bytes)
				.map_err(|_| P256Error::FailedToDeserializePublicKey)?;

		let sender_public_typed = SenderPublic(&ephemeral_sender_public_bytes);
		let receiver_encoded_point =
			self.private.public_key().to_encoded_point(false);
		let receiver_public_typed =
			ReceiverPublic(receiver_encoded_point.as_ref());

		let cipher = create_cipher(
			&self.private,
			&ephemeral_sender_public,
			&sender_public_typed,
			&receiver_public_typed,
		)?;

		let aad = create_additional_associated_data(
			&sender_public_typed,
			&receiver_public_typed,
			nonce.as_ref(),
		);
		let payload = Payload { aad: &aad, msg: &encrypted_message };

		cipher
			.decrypt(nonce, payload)
			.map_err(|_| P256Error::AesGcm256DecryptError)
	}

	/// Get the public key.
	#[must_use]
	pub fn public_key(&self) -> P256EncryptPublic {
		P256EncryptPublic { public: self.private.public_key() }
	}

	/// Deserialize key from raw scalar byte slice.
	pub fn from_bytes(bytes: &[u8]) -> Result<Self, P256Error> {
		Ok(Self {
			private: SecretKey::from_be_bytes(bytes)
				.map_err(|_| P256Error::FailedToReadSecret)?,
		})
	}

	/// Serialize key to raw scalar byte slice.
	#[must_use]
	pub fn to_bytes(&self) -> Vec<u8> {
		self.private.to_be_bytes().to_vec()
	}
}

/// P256 Public key.
#[cfg_attr(any(feature = "mock", test), derive(Clone, PartialEq))]
pub struct P256EncryptPublic {
	public: PublicKey,
}

impl P256EncryptPublic {
	/// Encrypt a message to this public key.
	pub fn encrypt(&self, message: &[u8]) -> Result<Vec<u8>, P256Error> {
		let ephemeral_sender_private = SecretKey::random(&mut OsRng);
		let ephemeral_sender_public: [u8; PUB_KEY_LEN_UNCOMPRESSED] =
			ephemeral_sender_private
				.public_key()
				.to_encoded_point(false)
				.as_ref()
				.try_into()
				.map_err(|_| {
					P256Error::FailedToCoercePublicKeyToIntendedLength
				})?;

		let sender_public_typed = SenderPublic(&ephemeral_sender_public);
		let receiver_encoded_point = self.public.to_encoded_point(false);
		let receiver_public_typed =
			ReceiverPublic(receiver_encoded_point.as_ref());

		let cipher = create_cipher(
			&ephemeral_sender_private,
			&self.public,
			&sender_public_typed,
			&receiver_public_typed,
		)?;

		let nonce = {
			let random_bytes =
				crate::non_zero_bytes_os_rng::<BITS_96_AS_BYTES>();
			*Nonce::from_slice(&random_bytes)
		};

		let aad = create_additional_associated_data(
			&sender_public_typed,
			&receiver_public_typed,
			nonce.as_ref(),
		);
		let payload = Payload { aad: &aad, msg: message };

		let encrypted_message = cipher
			.encrypt(&nonce, payload)
			.map_err(|_| P256Error::AesGcm256EncryptError)?;

		let nonce = nonce
			.try_into()
			.map_err(|_| P256Error::FailedToCoerceNonceToIntendedLength)?;

		let envelope =
			Envelope { nonce, ephemeral_sender_public, encrypted_message };

		envelope.try_to_vec().map_err(|_| P256Error::FailedToSerializeEnvelope)
	}

	/// Serialize to SEC1 encoded point, not compressed.
	#[must_use]
	pub fn to_bytes(&self) -> Box<[u8]> {
		let sec1_encoded_point = self.public.to_encoded_point(false);
		sec1_encoded_point.to_bytes()
	}

	/// Deserialize from a SEC1 encoded point, not compressed.
	pub fn from_bytes(bytes: &[u8]) -> Result<Self, P256Error> {
		Ok(Self {
			public: PublicKey::from_sec1_bytes(bytes)
				.map_err(|_| P256Error::FailedToReadPublicKey)?,
		})
	}
}

// Types for helper function parameters to help prevent fat finger mistakes.
struct SenderPublic<'a>(&'a [u8]);
struct ReceiverPublic<'a>(&'a [u8]);

// Helper function to create the `Aes256Gcm` cypher.
fn create_cipher(
	private: &SecretKey,
	public: &PublicKey,
	ephemeral_sender_public: &SenderPublic,
	receiver_public: &ReceiverPublic,
) -> Result<Aes256Gcm, P256Error> {
	let shared_secret =
		diffie_hellman(private.to_nonzero_scalar(), public.as_affine());

	// To help with entropy and add domain context, we do
	// `sender_public||receiver_public||shared_secret` as the pre-image for the
	// shared key.
	let pre_image: Vec<u8> = ephemeral_sender_public
		.0
		.iter()
		.chain(receiver_public.0)
		.chain(shared_secret.raw_secret_bytes())
		.copied()
		.collect();

	let mut mac = <HmacSha512 as KeyInit>::new_from_slice(&pre_image[..])
		.expect("hmac can take a key of any size");
	mac.update(&pre_image);
	let shared_key = mac.finalize().into_bytes();

	Aes256Gcm::new_from_slice(&shared_key[..AES256_KEY_LEN])
		.map_err(|_| P256Error::FailedToCreateAes256GcmCipher)
}

// Helper function to create the additional associated data (AAD). The data is
// of the form `sender_public||receiver_public||nonce`.
fn create_additional_associated_data(
	ephemeral_sender_public: &SenderPublic,
	receiver_public: &ReceiverPublic,
	nonce: &[u8],
) -> Vec<u8> {
	ephemeral_sender_public
		.0
		.iter()
		.chain(receiver_public.0)
		.chain(nonce)
		.copied()
		.collect()
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn basic_encrypt_decrypt_works() {
		let alice_pair = P256EncryptPair::generate();
		let alice_public = alice_pair.public_key();

		let plaintext = b"rust test message";

		let serialized_envelope = alice_public.encrypt(plaintext).unwrap();

		let decrypted = alice_pair.decrypt(&serialized_envelope).unwrap();

		assert_eq!(decrypted, plaintext);
	}

	#[test]
	fn wrong_receiver_cannot_decrypt() {
		let alice_pair = P256EncryptPair::generate();
		let alice_public = alice_pair.public_key();

		let plaintext = b"rust test message";

		let serialized_envelope = alice_public.encrypt(plaintext).unwrap();

		let bob_pair = P256EncryptPair::generate();

		assert_eq!(
			bob_pair.decrypt(&serialized_envelope).unwrap_err(),
			P256Error::AesGcm256DecryptError
		);
	}

	#[test]
	fn tampered_encrypted_message_fails() {
		let alice_pair = P256EncryptPair::generate();
		let alice_public = alice_pair.public_key();

		let plaintext = b"rust test message";

		let serialized_envelope = alice_public.encrypt(plaintext).unwrap();

		let mut envelope =
			Envelope::try_from_slice(&serialized_envelope).unwrap();

		envelope.encrypted_message.push(0);
		let tampered_envelope = envelope.try_to_vec().unwrap();

		assert_eq!(
			alice_pair.decrypt(&tampered_envelope).unwrap_err(),
			P256Error::AesGcm256DecryptError
		);
	}

	#[test]
	fn tampered_nonce_errors() {
		let alice_pair = P256EncryptPair::generate();
		let alice_public = alice_pair.public_key();

		let plaintext = b"rust test message";

		let serialized_envelope = alice_public.encrypt(plaintext).unwrap();

		let mut envelope =
			Envelope::try_from_slice(&serialized_envelope).unwrap();

		// Alter the first byte of the nonce.
		if envelope.nonce[0] == 0 {
			envelope.nonce[0] = 1;
		} else {
			envelope.nonce[0] = 0;
		};
		let tampered_envelope = envelope.try_to_vec().unwrap();

		assert_eq!(
			alice_pair.decrypt(&tampered_envelope).unwrap_err(),
			P256Error::AesGcm256DecryptError
		);
	}

	#[test]
	fn tampered_ephemeral_sender_key_errors() {
		let alice_pair = P256EncryptPair::generate();
		let alice_public = alice_pair.public_key();

		let plaintext = b"rust test message";

		let serialized_envelope = alice_public.encrypt(plaintext).unwrap();

		let mut envelope =
			Envelope::try_from_slice(&serialized_envelope).unwrap();

		// Alter the first byte of the sender's public key.
		if envelope.ephemeral_sender_public[0] == 0 {
			envelope.ephemeral_sender_public[0] = 1;
		} else {
			envelope.ephemeral_sender_public[0] = 0;
		};
		let tampered_envelope = envelope.try_to_vec().unwrap();

		assert_eq!(
			alice_pair.decrypt(&tampered_envelope).unwrap_err(),
			P256Error::FailedToDeserializePublicKey
		);
	}

	#[test]
	fn tampered_envelope_errors() {
		let alice_pair = P256EncryptPair::generate();
		let alice_public = alice_pair.public_key();

		let plaintext = b"rust test message";

		let mut serialized_envelope = alice_public.encrypt(plaintext).unwrap();
		// Given borsh encoding, this should be a byte in the nonce. We insert a
		// byte and shift everthing after, making the nonce too long.
		serialized_envelope.insert(BITS_96_AS_BYTES, 0xff);

		assert_eq!(
			alice_pair.decrypt(&serialized_envelope).unwrap_err(),
			P256Error::FailedToDeserializeEnvelope
		);
	}

	#[test]
	fn public_key_roundtrip_bytes() {
		let alice_pair = P256EncryptPair::generate();
		let alice_public = alice_pair.public_key();

		let public_key_bytes = alice_public.to_bytes();
		let alice_public2 =
			P256EncryptPublic::from_bytes(&public_key_bytes).unwrap();

		let plaintext = b"rust test message";

		let serialized_envelope = alice_public2.encrypt(plaintext).unwrap();

		let decrypted = alice_pair.decrypt(&serialized_envelope).unwrap();

		assert_eq!(decrypted, plaintext);
	}

	#[test]
	fn private_key_roundtrip_bytes() {
		let pair = P256EncryptPair::generate();
		let raw_secret1 = pair.to_bytes();

		let pair2 = P256EncryptPair::from_bytes(&raw_secret1).unwrap();
		let raw_secret2 = pair2.to_bytes();

		assert_eq!(raw_secret1, raw_secret2);
	}
}