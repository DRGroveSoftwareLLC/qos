fn main() {
	for i in 0..3 {
		std::thread::sleep(std::time::Duration::from_millis(i));
	}

	std::fs::write(qos_test::PIVOT_OK_SUCCESS_FILE, b"contents").unwrap();
}