//! The coordinator is responsible for initializing the enclave's primary
//! processes. Concretely it spawns the enclave server and launches the "pivot"
//! executable once it becomes available in the file system.
//!
//! The pivot is an executable the enclave runs to initialize the secure
//! applications.
use std::{path::Path, process::Command};

use crate::{cli::EnclaveOptions, protocol::Executor, server::SocketServer};

pub struct Coordinator;
impl Coordinator {
	/// Run the coordinator.
	pub fn execute(opts: EnclaveOptions) {
		let secret_file = opts.secret_file();
		let pivot_file = opts.pivot_file();

		std::thread::spawn(move || {
			let executor = Executor::new(
				opts.nsm(),
				opts.secret_file(),
				opts.pivot_file(),
			);
			SocketServer::listen(opts.addr(), executor).unwrap();
		});

		// Check if the enclaves secret and the pivot executable is loaded.
		loop {
			let secret_file_exists = is_file(&secret_file);
			let pivot_file_exists = is_file(&pivot_file);

			if secret_file_exists && pivot_file_exists {
				break
			}

			std::thread::sleep(std::time::Duration::from_secs(1));
		}

		// "Pivot" to the executable by spawning a child process running the
		// executable.
		let mut pivot = Command::new(pivot_file);
		let mut child_process =
			pivot.spawn().expect("Process failed to execute...");

		// Child process restart logic
		loop {
			let status = child_process
				.wait()
				.expect("Pivot executable never started...");
			if status.success() {
				println!("Pivot executable exited successfully ...");
				break
			} else {
				println!(
					"Re-spawning pivot executable child process - {}",
					status
				);
				child_process =
					pivot.spawn().expect("Process failed to execute ...");
			}
		}

		println!("Coordinator exiting ...");
	}
}

fn is_file(path: &str) -> bool {
	Path::new(path).exists()
}

// See qos-test/tests/coordinator for tests
