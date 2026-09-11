# 契约基线 fixture

从**运行中的 new-api 实例**录制的真实请求/响应,是 L3 契约测试的判定基准。

## 为什么不用设计文档做基准

`doc/test-design.md` §1 有完整论述。简言之:v1.0 的设计文档带着 4 处契约偏差
(camelCase 字段名、0 起分页、错误的计费公式、断链的端点清单)通过了首轮人工审查。
按文档写测试会把偏差固化成"正确行为"。

## 录制

```bash
NEWAPI_BASE_URL=http://localhost:3000 \
NEWAPI_ADMIN_TOKEN=<access_token> \
bash test/env/record-baseline.sh
```

## 文件格式

```json
{
  "recorded_from": "new-api",
  "recorded_at":   "2026-09-10T10:00:00Z",
  "newapi_version": "<VERSION 文件内容>",
  "endpoint": "GET /api/user/self",
  "request":  { "method": "GET", "path": "/api/user/self", "headers": {}, "body": null },
  "response": { "status": 200, "headers": {}, "body": {} }
}
```

流式端点额外带 `response.chunks`(字符串数组,每个元素是一行 SSE 事件)。

## 测试失败时怎么判断

**先看 `recorded_at` 与 `newapi_version`**:

| 情况 | 判断 | 处理 |
|---|---|---|
| 基线是近期录制,sea-weir 刚改过代码 | 我方回归 | 修实现 |
| 基线很旧,new-api 已升级多个版本 | 上游演进 | 重录基线,评估是否跟进 |
| 基线缺失 | 未录制 | 跑 record-baseline.sh |

## 清单

见 `cases/09-contract-admin.md` 与 `cases/10-contract-relay.md` 末尾的
「待录制的 fixture 清单」。

## 注意

- 录制时**不要用生产实例**,响应里可能含真实用户数据
- 录制脚本会对 key / token / email 做脱敏,但仍应人工复核后入库
- `*.local.json` 已在 .gitignore 中,可用于本地临时基线
