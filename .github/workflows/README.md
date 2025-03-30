# GitHub Actions 构建流程

## 构建 Debian 包

本项目使用 GitHub Actions 自动构建 Debian 包。构建流程配置在 `build-debian.yml` 文件中。

### 触发方式

构建可以通过以下方式触发：

1. 推送代码到 `main` 分支
2. 创建新的标签（以 `v` 开头，如 `v1.0.0`）
3. 创建针对 `main` 分支的 Pull Request
4. 手动在 GitHub Actions 界面触发

### 发布版本

当您创建新标签（如 `v1.0.0`）并推送到 GitHub 时，工作流会：

1. 构建 Rust 项目
2. 创建 Debian 包
3. 自动创建 GitHub Release
4. 将 Debian 包附加到该 Release

### 手动下载构建产物

即使没有创建 Release，您也可以从 GitHub Actions 的构建记录中下载 Debian 包：

1. 转到项目的 "Actions" 标签页
2. 选择最新的构建记录
3. 在构建详情页面的底部，找到 "Artifacts" 部分
4. 下载 "api-manager-debian" 文件

### 安装 Debian 包

下载 Debian 包后，可以使用以下命令安装：

```bash
sudo dpkg -i api-manager_版本号.deb
```

如果有依赖问题，可以运行：

```bash
sudo apt-get install -f
``` 