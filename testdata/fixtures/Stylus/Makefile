CRATES := $(wildcard crates/*)
WASMS  := $(foreach crate,$(CRATES),$(wildcard $(crate)/target/wasm32-unknown-unknown/release/*.wasm))
COPIED := $(notdir $(WASMS))

.PHONY: all check build clean

all: check build postprocess

# Run cargo stylus check in each sub-crate
check:
	@for dir in $(CRATES); do \
		echo "==> Checking $$dir"; \
		( cd $$dir && cargo stylus check ) || exit 1; \
	done

# Copy wasm artifacts to current dir
build:
	@for wasm in $(WASMS); do \
		echo "==> Copying $$wasm"; \
		cp $$wasm .; \
	done

# Run remove_reference_types.sh on copied .wasm and clean up .wat
postprocess:
	@for wasm in $(COPIED); do \
		echo "==> Processing $$wasm"; \
		./remove_reference_types.sh $$wasm || exit 1; \
	done
	@rm -f *.wat

# Cleanup copied wasm files and wat files
clean:
	rm -f $(COPIED) *.wat