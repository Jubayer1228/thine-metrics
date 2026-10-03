.PHONY: build test ui run score docker examples

build:
	cargo build -p thine-server

test:
	cargo test --workspace

ui:
	cd ui && npm install && npm run build

run: ui
	THINE_UI_DIR=ui/dist THINE_SEED_DEMO=true THINE_HOST=127.0.0.1 cargo run -p thine-server

dev:
	./scripts/dev.sh

score:
	./scripts/score_loop.sh

docker:
	docker compose up --build

examples:
	@echo "Python: python3 examples/python/emit_metrics.py"
	@echo "Go:     cd examples/go && go run ."
