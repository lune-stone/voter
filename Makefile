release:
	trunk build --release

run:
	trunk serve

clean:
	rm -rf dist/

check:
	cargo clippy -- -D clippy::expect_used -D clippy::panic  -D clippy::unwrap_used
