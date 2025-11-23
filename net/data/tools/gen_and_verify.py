#!/usr/bin/env python3
import hashlib
import os
import random
import sys

INPUT_PATH = "net/data/input_file.txt"
OUTPUT_PATH = "net/data/output_file.txt"
TEST_SIZE = 4096  # 4 KB，可通过命令行第二个参数覆盖


def gen_test_file(path: str, size: int) -> None:
    random.seed(0x13_37)
    data = bytearray(random.getrandbits(8) for _ in range(size))
    with open(path, "wb") as f:
        f.write(data)
    print(f"生成测试文件: {path} ({size} bytes)")


def sha256(path: str) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(8192), b""):
            h.update(chunk)
    return h.hexdigest()


def verify_output(input_path: str, output_path: str) -> None:
    if not os.path.exists(output_path):
        print(f"未找到接收文件: {output_path}")
        return
    in_hash = sha256(input_path)
    out_hash = sha256(output_path)
    if in_hash == out_hash:
        print(f"校验通过，SHA256={in_hash}")
    else:
        print(f"校验失败:\n  输入文件={in_hash}\n  输出文件={out_hash}")


def usage() -> None:
    print("用法:")
    print("  python3 tools/gen_and_verify.py generate [size]")
    print("  python3 tools/gen_and_verify.py test [input_path] [output_path]")
    print("  直接运行脚本进入交互模式")


def interactive() -> None:
    usage()
    while True:
        cmd = input("\n请输入命令(generate/test/quit): ").strip().lower()
        if not cmd:
            continue
        if cmd in {"quit", "exit", "q"}:
            print("已退出。")
            return
        if cmd.startswith("generate"):
            parts = cmd.split()
            if len(parts) >= 2:
                try:
                    size = int(parts[1])
                except ValueError:
                    print("大小必须是整数。")
                    continue
            else:
                size_input = input(f"请输入生成文件大小(默认 {TEST_SIZE}): ").strip()
                size = TEST_SIZE if not size_input else int(size_input)
            gen_test_file(INPUT_PATH, size)
        elif cmd.startswith("test"):
            parts = cmd.split()
            if len(parts) >= 3:
                input_path, output_path = parts[1], parts[2]
            else:
                input_path = input(f"请输入输入文件路径(默认 {INPUT_PATH}): ").strip() or INPUT_PATH
                output_path = input(f"请输入输出文件路径(默认 {OUTPUT_PATH}): ").strip() or OUTPUT_PATH
            verify_output(input_path, output_path)
        else:
            print("未知命令。")
            usage()


if __name__ == "__main__":
    if len(sys.argv) < 2:
        interactive()
        sys.exit(0)

    command = sys.argv[1].lower()

    if command == "generate":
        size = int(sys.argv[2]) if len(sys.argv) >= 3 else TEST_SIZE
        gen_test_file(INPUT_PATH, size)
    elif command == "test":
        input_path = sys.argv[2] if len(sys.argv) >= 3 else INPUT_PATH
        output_path = sys.argv[3] if len(sys.argv) >= 4 else OUTPUT_PATH
        verify_output(input_path, output_path)
    else:
        usage()
        sys.exit(1)