CARGO ?= cargo
DEFAULT_IFACE ?= en0

.PHONY: build
build:
	$(CARGO) build

.PHONY: run-send
run-send:
	@if [ -z "$(IFACE)" ]; then \
		read -p "请输入网卡名称 (默认 $(DEFAULT_IFACE)): " iface_input; \
		if [ -z "$$iface_input" ]; then iface="$(DEFAULT_IFACE)"; else iface="$$iface_input"; fi; \
	else \
		iface="$(IFACE)"; \
	fi; \
	if [ -z "$(DEST)" ]; then \
		read -p "请输入目标 MAC (格式 XX:XX:XX:XX:XX:XX): " dest_input; \
		dest="$$dest_input"; \
	else \
		dest="$(DEST)"; \
	fi; \
	if [ -z "$$dest" ]; then \
		echo "目标 MAC 不能为空"; exit 1; \
	fi; \
	sudo $(CARGO) run -- send $$iface $$dest

.PHONY: run-recv
run-recv:
	@if [ -z "$(IFACE)" ]; then \
		read -p "请输入网卡名称 (默认 $(DEFAULT_IFACE)): " iface_input; \
		if [ -z "$$iface_input" ]; then iface="$(DEFAULT_IFACE)"; else iface="$$iface_input"; fi; \
	else \
		iface="$(IFACE)"; \
	fi; \
	sudo $(CARGO) run -- recv $$iface

.PHONY: fmt
fmt:
	$(CARGO) fmt

.PHONY: clean
clean:
	$(CARGO) clean