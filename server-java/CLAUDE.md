# server-java/ — Java 服务端

## 概览

Spring Boot 3.3.8，主推实现。10 步校验链完整实现。

## 构建

**本机无 JDK 17/Maven，构建必须在 Docker 内进行**（`Dockerfile.server-java` 使用 `maven:3.9-eclipse-temurin-17`）。

本机有 JDK 时：

```bash
JAVA_HOME="/opt/homebrew/opt/openjdk" mvn test
```

> 系统 `java` 指向 JDK 8，直接调用 `mvn test` 会导致 surefire fork 崩溃。

## 关键约束

- `EditRecordPort` 使用 `PageResult<T>` 替代 Spring `Page`（禁止混用）
- 本地开发默认 H2 内存库（无需 `DATABASE_URL`）
- 生产使用 PostgreSQL（`DATABASE_URL` 必填）
- `X-Admin-Key` 鉴权，H2 / PostgreSQL 均支持管理端点

## 测试

```bash
JAVA_HOME="/opt/homebrew/opt/openjdk" mvn test
```

覆盖率目标 LINE ≥ 90%（当前 218 tests）。

## 端口

默认 `:8080`。
