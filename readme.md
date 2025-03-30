api-manager/
├── Cargo.toml          # 项目配置和依赖
├── src/
│   ├── main.rs         # 应用入口点
│   ├── lib.rs          # 库部分（可选）
│   ├── config/         # 配置文件处理
│   │   ├── mod.rs      # 配置模块导出
│   │   └── app.rs      # 应用配置
│   ├── routes/         # 路由定义
│   │   ├── mod.rs      # 路由模块导出
│   │   ├── api.rs      # API路由
│   │   └── web.rs      # Web路由（可选）
│   ├── handlers/       # 请求处理器
│   │   ├── mod.rs      # 处理器模块导出
│   │   ├── api/        # API处理器
│   │   └── web/        # Web处理器（可选）
│   ├── models/         # 数据模型
│   │   ├── mod.rs      # 模型模块导出
│   │   ├── user.rs     # 示例：用户模型
│   │   └── ...
│   ├── services/       # 业务逻辑服务
│   │   ├── mod.rs      # 服务模块导出
│   │   └── ...
│   ├── database/       # 数据库相关
│   │   ├── mod.rs      # 数据库模块导出
│   │   └── connection.rs # 数据库连接
│   ├── middlewares/    # 中间件
│   │   ├── mod.rs      # 中间件模块导出
│   │   └── ...
│   ├── errors/         # 错误处理
│   │   ├── mod.rs      # 错误模块导出
│   │   └── http.rs     # HTTP错误处理
│   └── utils/          # 实用工具
│       ├── mod.rs      # 工具模块导出
│       └── ...
├── migrations/         # 数据库迁移（如果使用SQLx等）
├── tests/              # 集成测试
├── .env                # 环境变量
└── .env.example        # 示例环境变量