use std::{fs, path::Path, process::Command};

use borsh::de::BorshDeserialize;
use qos_attest::nitro::unsafe_attestation_doc_from_der;
use qos_core::protocol::services::genesis::GenesisOutput;
use qos_crypto::{shamir::shares_reconstruct, RsaPair, RsaPub};
use integration::LOCAL_HOST;
use rand::{seq::SliceRandom, thread_rng};
use qos_test_primitives::{ChildWrapper, PathWrapper};

#[tokio::test]
async fn genesis_e2e() {
	let host_port = qos_test_primitives::find_free_port().unwrap();
	let tmp: PathWrapper = "/tmp/genesis-e2e".into();
	fs::create_dir_all(*tmp).unwrap();

	let usock: PathWrapper = "/tmp/genesis-e2e/genesis_e2e.sock".into();
	let secret_path: PathWrapper = "/tmp/genesis-e2e/genesis_e2e.secret".into();
	let pivot_path: PathWrapper = "/tmp/genesis-e2e/genesis_e2e.pivot".into();
	let manifest_path = "/tmp/genesis-e2e/manifest.manifest";

	let all_personal_dir: PathWrapper = "/tmp/genesis-e2e-personal".into();
	let genesis_dir: PathWrapper = "/tmp/genesis-e2e-genesis".into();

	let namespace = "quit-coding-to-vape";
	let attestation_doc_path =
		format!("{}/attestation_doc.genesis", *genesis_dir);
	let genesis_output_path = format!("{}/output.genesis", *genesis_dir);

	let personal_dir =
		|user: &str| format!("{}/{}-dir", *all_personal_dir, user);
	let get_key_paths = |user: &str| {
		(
			format!("{}.{}.setup.key", user, namespace),
			format!("{}.{}.setup.pub", user, namespace),
		)
	};

	let threshold = 2;
	let user1 = "user1";
	let (user1_private_setup, user1_public_setup) = get_key_paths(user1);

	let user2 = "user2";
	let (user2_private_setup, user2_public_setup) = get_key_paths(user2);

	let user3 = "user3";
	let (user3_private_setup, user3_public_setup) = get_key_paths(user3);

	// -- CLIENT Create 3 setup keys
	// Make sure the directory keys are getting written to already exist.
	for (user, private, public) in [
		(&user1, &user1_private_setup, &user1_public_setup),
		(&user2, &user2_private_setup, &user2_public_setup),
		(&user3, &user3_private_setup, &user3_public_setup),
	] {
		assert!(Command::new("../target/debug/qos_client")
			.args([
				"generate-setup-key",
				"--personal-dir",
				&personal_dir(user),
				"--namespace",
				namespace,
				"--alias",
				user,
			])
			.spawn()
			.unwrap()
			.wait()
			.unwrap()
			.success());
		assert!(Path::new(&personal_dir(user)).join(public).is_file());
		assert!(Path::new(&personal_dir(user)).join(private).is_file());
	}

	// Make the genesis dir
	fs::create_dir_all(*genesis_dir).unwrap();
	// Move the setup keys to the genesis dir - this will be the Genesis Set
	for (user, public) in [
		(&user1, &user1_public_setup),
		(&user2, &user2_public_setup),
		(&user3, &user3_public_setup),
	] {
		let from = Path::new(&personal_dir(user)).join(public);
		let to = Path::new(*genesis_dir).join(public);
		fs::copy(from, to).unwrap();
	}

	// -- ENCLAVE start enclave
	let mut _enclave_child_process: ChildWrapper =
		Command::new("../target/debug/qos_core")
			.args([
				"--usock",
				*usock,
				"--quorum-file",
				*secret_path,
				"--pivot-file",
				*pivot_path,
				"--mock",
				"--manifest-file",
				manifest_path,
			])
			.spawn()
			.unwrap()
			.into();

	// -- HOST start host
	let mut _host_child_process: ChildWrapper =
		Command::new("../target/debug/qos_host")
			.args([
				"--host-port",
				&host_port.to_string(),
				"--host-ip",
				LOCAL_HOST,
				"--usock",
				*usock,
			])
			.spawn()
			.unwrap()
			.into();

	// -- Make sure the enclave and host have time to boot
	qos_test_primitives::wait_until_port_is_bound(host_port);

	// -- CLIENT Run boot genesis, creating a genesis set from the setup keys in
	// the genesis dir
	assert!(Command::new("../target/debug/qos_client")
		.args([
			"boot-genesis",
			"--threshold",
			"2", // threshold
			"--genesis-dir",
			*genesis_dir,
			"--host-ip",
			LOCAL_HOST,
			"--host-port",
			&host_port.to_string(),
			"--unsafe-skip-attestation"
		])
		.spawn()
		.unwrap()
		.wait()
		.unwrap()
		.success());

	// -- Check files generated from the genesis boot
	drop(unsafe_attestation_doc_from_der(
		&fs::read(attestation_doc_path).unwrap(),
	));
	let genesis_output =
		GenesisOutput::try_from_slice(&fs::read(&genesis_output_path).unwrap())
			.unwrap();

	// -- Recreate the quorum key from the encrypted shares.
	let mut decrypted_shares: Vec<_> = genesis_output
		.member_outputs
		.iter()
		.map(|member| {
			let alias = &member.setup_member.alias;
			let (private_setup, _) = get_key_paths(alias);
			let setup_pair = RsaPair::from_pem_file(
				Path::new(&personal_dir(alias)).join(private_setup),
			)
			.unwrap();

			// Decrypt the personal key with the setup key
			let personal_key = RsaPair::from_der(
				&setup_pair
					.envelope_decrypt(&member.encrypted_personal_key)
					.unwrap(),
			)
			.unwrap();

			// Decrypt the share with the personal key
			personal_key
				.envelope_decrypt(&member.encrypted_quorum_key_share)
				.unwrap()
		})
		.collect();

	// Try recovering from a random permutation
	decrypted_shares.shuffle(&mut thread_rng());
	let reconstructed =
		RsaPair::from_der(&shares_reconstruct(&decrypted_shares[0..threshold]))
			.unwrap();
	assert_eq!(
		*reconstructed.public_key(),
		RsaPub::from_der(&genesis_output.quorum_key).unwrap()
	);

	// -- CLIENT make sure each user can run `after-genesis` against their
	// member output and setup key
	for user in [&user1, &user2, &user3] {
		assert!(Command::new("../target/debug/qos_client")
			.args([
				"after-genesis",
				"--personal-dir",
				&personal_dir(user),
				"--genesis-dir",
				*genesis_dir,
				"--pcr0",
				"0xff",
				"--pcr1",
				"0xff",
				"--pcr2",
				"0xff",
				"--unsafe-skip-attestation"
			])
			.spawn()
			.unwrap()
			.wait()
			.unwrap()
			.success());

		let personal_pub = Path::new(&personal_dir(user))
			.join(format!("{}.{}.personal.pub", user, namespace));
		let personal_key = Path::new(&personal_dir(user))
			.join(format!("{}.{}.personal.key", user, namespace));
		let share_path = Path::new(&personal_dir(user))
			.join(format!("{}.{}.share", user, namespace));
		// Read in the personal public and private key
		let public = RsaPub::from_pem_file(personal_pub).unwrap();
		let private = RsaPair::from_pem_file(personal_key).unwrap();
		assert_eq!(
			private.public_key_to_der().unwrap(),
			public.public_key_to_der().unwrap()
		);
		// Check the share is encrypted to personal key
		let share =
			private.envelope_decrypt(&fs::read(share_path).unwrap()).unwrap();
		// Cross check that the share belongs `decrypted_shares`, which we
		// created out of band in this test.
		assert!(decrypted_shares.contains(&share));
	}
}
