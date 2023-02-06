//! Attestation specific logic

#![forbid(unsafe_code)]
#![deny(clippy::all)]
#![warn(missing_docs, clippy::pedantic)]
#![allow(clippy::missing_errors_doc, clippy::module_name_repetitions)]

pub mod nitro;

/// Attestation error.
#[derive(Debug)]
pub enum AttestError {
	/// `webpki::Error` wrapper.
	WebPki(webpki::Error),
	/// Invalid certificate chain.
	InvalidCertChain(webpki::Error),
	/// `aws_nitro_enclaves_nsm_api::api::Error` wrapper.
	Nsm(aws_nitro_enclaves_nsm_api::api::Error),
	/// Invalid end entity certificate. In the case of Nitro this means the
	/// NSM's certificate was invalid.
	InvalidEndEntityCert,
	/// Invalid COSE Sign1 structure signature. In the case of Nitro this means
	/// the end entitys signature of the attestation doc was invalid.
	InvalidCOSESign1Signature,
	/// Invalid COSE Sign1 structure.
	InvalidCOSESign1Structure,
	/// Invalid hash digest.
	InvalidDigest,
	/// Invalid NSM module id.
	InvalidModuleId,
	/// Invalid PCR.
	InvalidPcr,
	/// Invalid certificate authority bundle.
	InvalidCABundle,
	/// Invalid time.
	InvalidTimeStamp,
	/// Invalid public key.
	InvalidPubKey,
	/// Invalid bytes.
	InvalidBytes,
	/// The NSM returned an unexpected response when querried
	UnexpectedNsmResponse(qos_nsm::types::NsmResponse),
	/// Error while decoding PEM.
	PemDecodingError,
	/// Error trying to decode the public key in a cert.
	FailedDecodeKeyFromCert,
	/// Error while trying to parse a cert.
	FailedToParseCert,
	/// User data is missing in the attestation doc.
	MissingUserData,
	/// User data (normally manifest hash) does not match the attestation doc.
	DifferentUserData,
	/// The attestation doc has a nonce when none was expected.
	UnexpectedAttestationDocNonce,
	/// The attestation doc does not contain a pcr0.
	MissingPcr0,
	/// The pcr3 in the attestation doc does not match.
	DifferentPcr0,
	/// The attestation doc does not have a pcr1.
	MissingPcr1,
	/// The attestation doc has a different pcr1.
	DifferentPcr1,
	/// The attestation doc does not have a pcr2.
	MissingPcr2,
	/// The attestation doc has a different pcr2.
	DifferentPcr2,
	/// The attestation doc does not have a pcr3.
	MissingPcr3,
	/// The attestation doc has a different pcr3.
	DifferentPcr3,
}

impl From<webpki::Error> for AttestError {
	fn from(e: webpki::Error) -> Self {
		Self::WebPki(e)
	}
}

impl From<aws_nitro_enclaves_nsm_api::api::Error> for AttestError {
	fn from(e: aws_nitro_enclaves_nsm_api::api::Error) -> Self {
		Self::Nsm(e)
	}
}

/// Get the current time in milliseconds based on the NSM attestation document.
pub fn current_time(
	nsm: &dyn qos_nsm::NsmProvider,
) -> Result<u64, AttestError> {
	let nsm_request = qos_nsm::types::NsmRequest::Attestation {
		user_data: None,
		nonce: None,
		public_key: None,
	};
	let fd = nsm.nsm_init();
	let nsm_response = nsm.nsm_process_request(fd, nsm_request);
	let nsm_response = match nsm_response {
		qos_nsm::types::NsmResponse::Attestation { document } => document,
		resp => return Err(AttestError::UnexpectedNsmResponse(resp)),
	};
	let attestation_document =
		nitro::unsafe_attestation_doc_from_der(&nsm_response)?;

	Ok(attestation_document.timestamp)
}
