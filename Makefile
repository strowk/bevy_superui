cargo-tests:
	cargo test

game-menu-superui-test:
	( cd examples/game_menu && cargo run -p superui_test_engine --bin superui_test )

superui-tests: game-menu-superui-test

verify-all: cargo-tests superui-tests

install-types:
	cargo run -p cargo-superui -- install --path examples/citadel
	cargo run -p cargo-superui -- install --path examples/game_menu
	cargo run -p cargo-superui -- install --path examples/horde
	cargo run -p cargo-superui -- install --path examples/todomvc_supersolid
	cargo run -p cargo-superui -- install --path examples/counter
