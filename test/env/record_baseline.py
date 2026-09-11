#!/usr/bin/env python3
"""从运行中的 new-api 实例录制契约基线。

由 `record-baseline.sh` 调用(该脚本负责参数校验与环境推导)。
用 Python 而非纯 bash,是因为要做 JSON 解析、递归脱敏与 SSE 分帧 ——
这些在 shell 里拼字符串很容易被响应体里的引号搞坏。

产出 `test/cases/fixtures/baseline/<name>.json`,格式见该目录 README。
"""

import argparse
import datetime
import json
import os
import re
import sys
import urllib.error
import urllib.request

# 字段名命中即打码。宁可多打,录制物入库后人工复核时容易发现打多了,
# 但打漏一个真实密钥就是事故。
SENSITIVE_KEY = re.compile(r"(key|token|secret|password|passwd|email|phone|access_code)", re.I)
# 值形态命中即打码(防止字段名没起对但值是密钥的情况)
SENSITIVE_VAL = re.compile(r"\bsk-[A-Za-z0-9_\-]{16,}")


def already_masked(val: str) -> bool:
    """上游自己已经脱敏过的展示值(含 `*`),不要再覆盖。

    令牌列表的 `key` 就是这种:它既是敏感字段名,又是**需要校验形状的契约内容**
    (前端直接展示这个字符串)。把它二次打码会让契约测试失去判定对象。
    """
    return "*" in val


def scrub(v):
    """递归脱敏。保留结构与类型 —— 契约测试比的是形状,不是取值。"""
    if isinstance(v, dict):
        out = {}
        for k, val in v.items():
            if (SENSITIVE_KEY.search(k) and isinstance(val, str) and val
                    and not already_masked(val)):
                out[k] = "<redacted>"
            else:
                out[k] = scrub(val)
        return out
    if isinstance(v, list):
        return [scrub(i) for i in v]
    if isinstance(v, str):
        return SENSITIVE_VAL.sub("<redacted>", v)
    return v


def request(method, url, headers, body=None, timeout=30):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(url, data=data, method=method, headers=headers)
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r:
            return r.status, dict(r.headers), r.read()
    except urllib.error.HTTPError as e:
        return e.code, dict(e.headers), e.read()
    except Exception as e:  # 连接失败等
        return 0, {}, str(e).encode()


# 响应头里只保留契约相关的,其余(Date/Server/Content-Length)是噪声
KEEP_HEADERS = {"content-type", "retry-after", "x-request-id", "request-id", "cache-control"}


def record(out_dir, name, method, path, base_url, headers, body, meta, stream=False):
    url = base_url.rstrip("/") + path
    status, resp_headers, raw = request(method, url, headers, body)

    kept = {k: v for k, v in resp_headers.items() if k.lower() in KEEP_HEADERS}

    entry = {
        "recorded_from": "new-api",
        "recorded_at": meta["now"],
        "newapi_version": meta["version"],
        "endpoint": f"{method} {path}",
        "request": {
            "method": method,
            "path": path,
            # 请求头不入库(含凭证),只记录用了哪些头名
            "header_names": sorted(headers.keys()),
            "body": scrub(body) if body else None,
        },
        "response": {"status": status, "headers": kept},
    }

    text = raw.decode("utf-8", errors="replace")
    if stream:
        # SSE:按行切分,保留 `data:` 与注释行的原始形态
        chunks = [ln for ln in text.splitlines() if ln.strip()]
        entry["response"]["chunks"] = [SENSITIVE_VAL.sub("<redacted>", c) for c in chunks]
        entry["response"]["body"] = None
    else:
        try:
            entry["response"]["body"] = scrub(json.loads(text))
        except json.JSONDecodeError:
            # 非 JSON(如静态资源、纯文本错误)保留截断的原文供人工判断
            entry["response"]["body"] = {"_non_json": text[:2000]}

    with open(os.path.join(out_dir, f"{name}.json"), "w", encoding="utf-8") as f:
        json.dump(entry, f, ensure_ascii=False, indent=2)
        f.write("\n")

    flag = "✔" if status else "✘"
    print(f"  {flag} {name:34} {method:6} {path:46} -> {status}")
    return status


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--base-url", required=True)
    ap.add_argument("--admin-token", required=True, help="new-api 系统访问令牌")
    ap.add_argument("--user-id", default="1", help="New-Api-User 头的值")
    ap.add_argument("--sk-token", default="", help="中继面用的 sk- 令牌(可选)")
    ap.add_argument("--lowpriv-token", default="", help="role=1 普通用户的访问令牌(可选)")
    ap.add_argument("--lowpriv-user-id", default="2")
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    os.makedirs(args.out, exist_ok=True)
    now = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")

    st, _, raw = request("GET", args.base_url.rstrip("/") + "/api/status", {})
    version = "unknown"
    if st == 200:
        try:
            version = json.loads(raw).get("data", {}).get("version", "unknown")
        except Exception:
            pass
    meta = {"now": now, "version": version}
    print(f"[record] {args.base_url}  version={version}")

    # 管理面:access token 路径**同样要求 New-Api-User 头**(与 session 路径一致),
    # 缺了会 401 —— 这一点在 CONTRACTS.md 的表述里不够明确,已实测确认。
    admin = {
        "Content-Type": "application/json",
        "Authorization": f"Bearer {args.admin_token}",
        "New-Api-User": args.user_id,
    }
    public = {"Content-Type": "application/json"}

    print("\n[record] 管理面")
    ok = fail = 0
    admin_cases = [
        ("api_user_self",          "GET", "/api/user/self",                      admin, None),
        ("api_user_list",          "GET", "/api/user/?p=1&page_size=10",         admin, None),
        ("api_token_list",         "GET", "/api/token/?p=1&page_size=10",        admin, None),
        ("api_channel_list",       "GET", "/api/channel/?p=1&page_size=10",      admin, None),
        ("api_log_self",           "GET", "/api/log/self?p=1&page_size=10",      admin, None),
        ("api_redemption_list",    "GET", "/api/redemption/?p=1&page_size=10",   admin, None),
        ("api_group_list",         "GET", "/api/group/",                         admin, None),
        ("api_models_list",        "GET", "/api/models/?p=1&page_size=10",       admin, None),
        ("api_data_self",          "GET", "/api/data/self",                      admin, None),
        ("api_option_list",        "GET", "/api/option/",                        admin, None),
        ("api_pricing_authed",     "GET", "/api/pricing",                        admin, None),
        # 公开端点
        ("api_status",             "GET", "/api/status",                         public, None),
        ("api_setup",              "GET", "/api/setup",                          public, None),
        ("api_notice",             "GET", "/api/notice",                         public, None),
        ("api_about",              "GET", "/api/about",                          public, None),
        ("api_pricing_anonymous",  "GET", "/api/pricing",                        public, None),
        ("api_ratio_config",       "GET", "/api/ratio_config",                   public, None),
        # 错误形状
        ("api_error_unauthorized", "GET", "/api/user/self",                      public, None),
        ("api_error_notfound",     "GET", "/api/channel/999999",                 admin, None),
    ]
    for name, m, p, h, b in admin_cases:
        if record(args.out, name, m, p, args.base_url, h, b, meta):
            ok += 1
        else:
            fail += 1

    # 角色不足。实测:管理面**不返回 403**,而是 HTTP 200 + success:false
    # (源码 middleware/auth.go authHelper:`role < minRole` 走 c.JSON(StatusOK,...))。
    # 403 只出现在中继面(如"无权访问 X 分组")。
    if args.lowpriv_token:
        low = {
            "Content-Type": "application/json",
            "Authorization": f"Bearer {args.lowpriv_token}",
            "New-Api-User": args.lowpriv_user_id,
        }
        print("\n[record] 角色闸门")
        for name, path in [
            ("api_error_insufficient_root",  "/api/option/"),       # RootAuth
            ("api_error_insufficient_admin", "/api/channel/"),      # AdminAuth
        ]:
            if record(args.out, name, "GET", path, args.base_url, low, None, meta):
                ok += 1
            else:
                fail += 1
    else:
        print("\n[record] (未提供 --lowpriv-token,跳过角色闸门用例)")

    # 业务错误(HTTP 200 + success:false)—— 契约里最容易被"改成 RESTful"破坏的一条
    print("\n[record] 业务错误形状")
    if record(args.out, "api_error_business", "POST", "/api/user/topup",
              args.base_url, admin, {"key": "INVALID-REDEMPTION-CODE"}, meta):
        ok += 1
    else:
        fail += 1

    print("\n[record] 中继面")
    relay_public = {"Content-Type": "application/json"}
    relay_cases = [
        ("relay_error_no_token",  "POST", "/v1/chat/completions", relay_public,
         {"model": "gpt-4", "messages": [{"role": "user", "content": "hi"}]}, False),
        ("relay_error_claude_no_token", "POST", "/v1/messages", relay_public,
         {"model": "claude-3-5-sonnet", "max_tokens": 16,
          "messages": [{"role": "user", "content": "hi"}]}, False),
    ]
    if args.sk_token:
        sk = {"Content-Type": "application/json", "Authorization": f"Bearer {args.sk_token}"}
        relay_cases += [
            ("relay_models_list", "GET", "/v1/models", sk, None, False),
            ("relay_model_detail", "GET", "/v1/models/gpt-4", sk, None, False),
            ("dashboard_billing_subscription", "GET",
             "/dashboard/billing/subscription", sk, None, False),
            ("dashboard_billing_usage", "GET",
             "/dashboard/billing/usage?start_date=2026-09-01&end_date=2026-09-10",
             sk, None, False),
            # 无可用渠道 -> model_not_found,这是中继面最重要的错误形状之一
            ("relay_error_no_channel", "POST", "/v1/chat/completions", sk,
             {"model": "definitely-nonexistent-model",
              "messages": [{"role": "user", "content": "hi"}]}, False),
            ("relay_error_mj", "POST", "/mj/submit/imagine", sk, {"prompt": "test"}, False),
            # 契约测试按格式命名引用这三份(与上面按场景命名的是同一批端点,
            # 保留两套名字以便分别表达"按场景"与"按出口格式"两个视角)
            ("relay_error_openai", "POST", "/v1/chat/completions", sk,
             {"model": "definitely-nonexistent-model",
              "messages": [{"role": "user", "content": "hi"}]}, False),
            ("relay_error_claude", "POST", "/v1/messages", sk,
             {"model": "definitely-nonexistent-model", "max_tokens": 16,
              "messages": [{"role": "user", "content": "hi"}]}, False),
            ("relay_claude_messages", "POST", "/v1/messages", sk,
             {"model": "gpt-4", "max_tokens": 16,
              "messages": [{"role": "user", "content": "hi"}]}, False),
            # 占位端点。实测有两种行为,都要录:
            #   POST 带 body -> 501 api_not_implemented(真正到达 RelayNotImplemented)
            #   GET  无 body -> 400,被 Distribute 的 body 解析拦在 handler 之前
            ("relay_not_implemented", "POST", "/v1/fine-tunes", sk, {"model": "gpt-4"}, False),
            ("relay_not_implemented_get", "GET", "/v1/files", sk, None, False),
            # 成功路径(需要一个可用渠道;无渠道时会录成 503 model_not_found)
            ("relay_openai_chat_nonstream", "POST", "/v1/chat/completions", sk,
             {"model": "gpt-4", "messages": [{"role": "user", "content": "hi"}]}, False),
            ("relay_openai_chat_stream", "POST", "/v1/chat/completions", sk,
             {"model": "gpt-4", "messages": [{"role": "user", "content": "hi"}],
              "stream": True}, True),
            ("relay_openai_chat_stream_usage", "POST", "/v1/chat/completions", sk,
             {"model": "gpt-4", "messages": [{"role": "user", "content": "hi"}],
              "stream": True, "stream_options": {"include_usage": True}}, True),
            ("sse_openai_basic", "POST", "/v1/chat/completions", sk,
             {"model": "gpt-4", "messages": [{"role": "user", "content": "hi"}],
              "stream": True}, True),
        ]
    else:
        print("  (未提供 --sk-token,跳过需令牌的中继端点)")

    for name, m, p, h, b, stream in relay_cases:
        if record(args.out, name, m, p, args.base_url, h, b, meta, stream=stream):
            ok += 1
        else:
            fail += 1

    print(f"\n[record] 完成:{ok} 成功 / {fail} 失败,输出 {args.out}")
    print("[record] 请人工复核脱敏结果后再入库")
    return 0 if fail == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
