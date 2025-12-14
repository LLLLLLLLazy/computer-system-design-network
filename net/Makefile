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

.PHONY: udp-send
udp-send: build
	@if [ -z "$(IFACE)" ]; then \
		read -p "请输入网卡名称 (默认 $(DEFAULT_IFACE)): " iface_input; \
		if [ -z "$$iface_input" ]; then iface="$(DEFAULT_IFACE)"; else iface="$$iface_input"; fi; \
	else \
		iface="$(IFACE)"; \
	fi; \
	if [ -z "$(DEST_IP)" ]; then \
		read -p "请输入目标 IP (a.b.c.d): " dest_ip_input; dest_ip="$$dest_ip_input"; \
	else \
		dest_ip="$(DEST_IP)"; \
	fi; \
	if [ -z "$(DEST_PORT)" ]; then \
		read -p "请输入目标端口: " dest_port_input; dest_port="$$dest_port_input"; \
	else \
		dest_port="$(DEST_PORT)"; \
	fi; \
	if [ -z "$(FILE)" ]; then \
		read -p "请输入要发送的文件路径 (默认 ./data/input_file.txt): " file_input; \
		if [ -z "$$file_input" ]; then file="./data/input_file.txt"; else file="$$file_input"; fi; \
	else \
		file="$(FILE)"; \
	fi; \
	src_port_opt=""; \
	if [ -n "$(SRC_PORT)" ]; then src_port_opt="$(SRC_PORT)"; fi; \
	if [ -n "$$src_port_opt" ]; then \
		$(SUDO) ./target/debug/net udp-send $$iface $$dest_ip $$dest_port $$file $$src_port_opt; \
	else \
		$(SUDO) ./target/debug/net udp-send $$iface $$dest_ip $$dest_port $$file; \
	fi

.PHONY: udp-recv
udp-recv: build
	@if [ -z "$(IFACE)" ]; then \
		read -p "请输入网卡名称 (默认 $(DEFAULT_IFACE)): " iface_input; \
		if [ -z "$$iface_input" ]; then iface="$(DEFAULT_IFACE)"; else iface="$$iface_input"; fi; \
	else \
		iface="$(IFACE)"; \
	fi; \
	if [ -z "$(LISTEN_PORT)" ]; then \
		read -p "请输入监听端口: " listen_port_input; listen_port="$$listen_port_input"; \
	else \
		listen_port="$(LISTEN_PORT)"; \
	fi; \
	if [ -z "$(OUTPUT)" ]; then \
		read -p "请输入输出文件路径 (默认 ./data/output_file.txt): " output_input; \
		if [ -z "$$output_input" ]; then output="./data/output_file.txt"; else output="$$output_input"; fi; \
	else \
		output="$(OUTPUT)"; \
	fi; \
	$(SUDO) ./target/debug/net udp-recv $$iface $$listen_port $$output

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

.PHONY: test
test:
	$(CARGO) test -- --nocapture

# 前端开发预览（SvelteKit dev）
# 用法：make frontend-dev [PORT=5173]
.PHONY: frontend-dev
frontend-dev:
	@cd ../web && PORT=$${PORT:-5173} npm run dev -- --host --port $${PORT:-5173}

# 前端构建（生产静态资源）
.PHONY: frontend-build
frontend-build:
	@cd ../web && npm run build

# 前端本地预览已构建产物
.PHONY: frontend-preview
frontend-preview:
	@cd ../web && npm run preview

# 一键启动状态服务 + 前端开发服（需要两个并发任务）
# 默认端口：状态服务 5174，前端 5173
.PHONY: dev-all
dev-all:
	@STATUS_PORT=$${STATUS_PORT:-5174}; \
	PORT=$${PORT:-5173}; \
	echo "启动 status_server ($$STATUS_PORT) 与前端 dev ($$PORT)"; \
	(STATUS_PORT=$$STATUS_PORT cd ../net && cargo run --bin status_server) & \
	(PORT=$$PORT STATUS_PORT=$$STATUS_PORT cd ../web && npm run dev -- --host --port $$PORT); \
	wait

# 调试目标，显示检测到的系统和使用的命令
.PHONY: debug
debug:
	@echo "检测到的操作系统: $(UNAME_S)"
	@echo "将使用的SUDO命令: $(SUDO)"
	@echo "默认网卡接口: $(DEFAULT_IFACE)"