# Makefile for treebeard (the forest's copy).
# `make` alone does nothing hasty; it asks first.

HOOM   ?= hoom
SEASON ?= spring

.PHONY: all consider build lint entmoot release search-entwives clean

all: consider

consider:
	@echo "$(HOOM). Considering. Run 'make build' when you are sure."

build: lint
	cargo build --release    # the only fast thing in this forest

lint:
	@echo "checking every name is long enough to mean something..."
	cargo run --bin entish -- ../..

# Convene an Entmoot. Three days minimum; see .github/workflows/entmoot-ci.yml.
entmoot:
	@echo "convening at Derndingle; please allow three days."
	@for day in 1 2 3; do echo "  day $$day: $(HOOM)"; done
	@echo "we have decided."

release: entmoot build
	@test "$(SEASON)" = "spring" || { echo "not in $(SEASON). wait."; exit 1; }
	@echo "released. ring recorded in ../old-growth/."

search-entwives:
	@echo "searching: see ../entwives/search-index.json"
	@echo "found: not yet."

# There is no 'fell' target. There will be no 'fell' target.
clean:
	@echo "sweeping leaves only. nothing living is removed."
