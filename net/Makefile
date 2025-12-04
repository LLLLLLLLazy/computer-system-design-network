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
	if [ -z "$(DEST_IP)" ]; then \
		read -p "请输入目标 IP (a.b.c.d): " dest_ip_input; \
		dest_ip="$$dest_ip_input"; \
	else \
		dest_ip="$(DEST_IP)"; \
	fi; \
	if [ -z "$$dest_ip" ]; then \
		echo "目标 IP 不能为空"; exit 1; \
	fi; \
	if [ -z "$(PROTOCOL)" ]; then \
		read -p "请输入协议号 (默认0，可留空): " proto_input; \
		if [ -z "$$proto_input" ]; then proto=0; else proto="$$proto_input"; fi; \
	else \
		proto="$(PROTOCOL)"; \
	fi; \
	if [ -z "$(DEST_MAC)" ]; then \
		read -p "可选：手动指定目标 MAC (留空表示使用 ARP 自动解析): " dest_mac_input; \
		dest_mac="$$dest_mac_input"; \
	else \
		dest_mac="$(DEST_MAC)"; \
	fi; \
	if [ -n "$$dest_mac" ]; then \
		$(SUDO) ./target/debug/net send $$iface $$dest_ip $$proto $$dest_mac; \
	else \
		$(SUDO) ./target/debug/net send $$iface $$dest_ip $$proto; \
	fi

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

.PHONY: test-arp
test-arp:
	$(CARGO) test enternet::arp::tests::subnet_detection

# 调试目标，显示检测到的系统和使用的命令
.PHONY: debug
debug:
	@echo "检测到的操作系统: $(UNAME_S)"
	@echo "将使用的SUDO命令: $(SUDO)"
	@echo "默认网卡接口: $(DEFAULT_IFACE)"