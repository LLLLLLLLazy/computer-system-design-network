# 检测操作系统
UNAME_S := $(shell uname -s)

ifeq ($(UNAME_S),Darwin)
    SUDO := sudo
    DEFAULT_IFACE ?= en0
else
    SUDO := sudo
    DEFAULT_IFACE ?= wlp0s20f3
endif

CARGO ?= cargo

.PHONY: build
build:
	$(CARGO) build

.PHONY: run-send
run-send: build
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
	$(SUDO) ./target/debug/net send $$iface $$dest

.PHONY: run-recv
run-recv: build
	@if [ -z "$(IFACE)" ]; then \
		read -p "请输入网卡名称 (默认 $(DEFAULT_IFACE)): " iface_input; \
		if [ -z "$$iface_input" ]; then iface="$(DEFAULT_IFACE)"; else iface="$$iface_input"; fi; \
	else \
		iface="$(IFACE)"; \
	fi; \
	$(SUDO) ./target/debug/net recv $$iface

.PHONY: fmt
fmt:
	$(CARGO) fmt

.PHONY: clean
clean:
	$(CARGO) clean

# 调试目标，显示检测到的系统和使用的命令
.PHONY: debug
debug:
	@echo "检测到的操作系统: $(UNAME_S)"
	@echo "将使用的SUDO命令: $(SUDO)"
	@echo "默认网卡接口: $(DEFAULT_IFACE)"