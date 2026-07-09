# Local build / CI parity for Cortex
.PHONY: help all build release test lint fmt clippy eval smoke deny python-test ci docker clean

CARGO ?= cargo
PYTHON ?= python3

help:
	@echo "Cortex make targets:"
	@echo "  all          build + test"
	@echo "  build        debug build"
	@echo "  release      release cortex-cli"
	@echo "  test         cargo test --workspace"
	@echo "  lint         fmt check + clippy -D warnings"
	@echo "  fmt          cargo fmt --all"
	@echo "  clippy       cargo clippy --workspace"
	@echo "  eval         cortex eval run"
	@echo "  smoke        scripts/smoke_agent.sh"
	@echo "  deny         cargo deny check (requires cargo-deny)"
	@echo "  python-test  pytest in sdks/python venv"
	@echo "  ci           full local CI suite"
	@echo "  docker       docker build -t cortex:local ."
	@echo "  clean        cargo clean"

all: build test

build:
	$(CARGO) build --workspace

release:
	$(CARGO) build --release -p cortex-cli

test:
	$(CARGO) test --workspace --all-targets

fmt:
	$(CARGO) fmt --all

clippy:
	$(CARGO) clippy --workspace --all-targets -- -D warnings

lint: fmt-check clippy

fmt-check:
	$(CARGO) fmt --all -- --check

eval:
	$(CARGO) build -p cortex-cli --quiet
	$(CARGO) run -q -p cortex-cli -- eval run --dir evals

smoke:
	./scripts/smoke_agent.sh

deny:
	$(CARGO) deny check

python-test:
	cd sdks/python && \
	  (test -d .venv || $(PYTHON) -m venv .venv) && \
	  .venv/bin/pip install -q -e ".[dev]" && \
	  .venv/bin/pytest -q

ci: lint test eval smoke python-test
	@echo "OK: local CI suite passed"

docker:
	docker build -t cortex:local .

clean:
	$(CARGO) clean
