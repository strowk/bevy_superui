cargo-tests:
	cargo test

game-menu-superui-test:
	( cd examples/game_menu && cargo run -p superui_test_engine --bin superui_test )

superui-tests: game-menu-superui-test

verify-all: cargo-tests superui-tests
