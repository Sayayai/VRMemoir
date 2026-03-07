# 使用包含 MinGW 的 Rust 映像进行 Windows 交叉编译
FROM rust:latest

# 安装必要的 64 位 Windows 交叉编译工具 + Opus/WASAPI 构建依赖
RUN apt-get update && apt-get install -y \
    gcc-mingw-w64-x86-64 \
    cmake \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# 添加 Windows GNU 目标
RUN rustup target add x86_64-pc-windows-gnu

WORKDIR /app

# 默认命令：构建项目并将可执行文件复制到挂载的目录  docker compose run --rm builder
# 使用 --release 以获得最佳性能，并剥离符号以减小体积
CMD ["cargo", "build", "--release", "--target", "x86_64-pc-windows-gnu"]
