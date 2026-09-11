/**
 * L3 契约 — 前端类型与录制基线的一致性。用例:`test/cases/14-frontend.md`。
 *
 * 断言依据是 `test/cases/fixtures/baseline/` 中从 new-api 录制的真实响应,
 * 而不是设计文档 —— 这正是「字段 snake_case」契约的守门测试。
 */
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

interface BaselineFixture {
  recorded_from: string;
  endpoint: string;
  response: { status: number; body: Record<string, unknown> };
}

function loadBaseline(name: string): BaselineFixture {
  const path = resolve(process.cwd(), '..', '..', 'test', 'cases', 'fixtures', 'baseline', `${name}.json`);
  return JSON.parse(readFileSync(path, 'utf8')) as BaselineFixture;
}

const SNAKE_CASE = /^[a-z0-9]+(_[a-z0-9]+)*$/;

function nonSnakeKeys(value: unknown, prefix = ''): string[] {
  if (Array.isArray(value)) {
    return value.flatMap((item, i) => nonSnakeKeys(item, `${prefix}[${i}]`));
  }
  if (value && typeof value === 'object') {
    const out: string[] = [];
    for (const [key, child] of Object.entries(value as Record<string, unknown>)) {
      if (!SNAKE_CASE.test(key)) {
        out.push(`${prefix}${prefix ? '.' : ''}${key}`);
      }
      out.push(...nonSnakeKeys(child, `${prefix}${prefix ? '.' : ''}${key}`));
    }
    return out;
  }
  return [];
}

describe('前端契约基线', () => {
  it('基线为真实录制(带元信息)', () => {
    const fixture = loadBaseline('api_user_self');
    expect(fixture.recorded_from).toBe('new-api');
    expect(fixture.endpoint).toBe('GET /api/user/self');
  });

  it('TC-UNI-FE-CONTRACT:管理面 /api/user/self 全字段 snake_case', () => {
    const fixture = loadBaseline('api_user_self');
    const bad = nonSnakeKeys(fixture.response.body);
    expect(bad, `发现非 snake_case 字段:${bad.join(', ')}`).toEqual([]);
  });

  it('TC-UNI-FE-CONTRACT:统一响应包裹为 success/message/data', () => {
    const body = loadBaseline('api_user_self').response.body;
    expect(body).toHaveProperty('success');
    expect(body).toHaveProperty('message');
    expect(body).toHaveProperty('data');
    expect(body.success).toBe(true);
  });

  it('TC-UNI-FE-CONTRACT:分页响应形状为 items/total/page/page_size(页码 1 起)', () => {
    const body = loadBaseline('api_token_list').response.body as {
      data: Record<string, unknown>;
    };
    for (const key of ['items', 'total', 'page', 'page_size']) {
      expect(body.data, `分页响应缺字段 ${key}`).toHaveProperty(key);
    }
    expect(body.data).not.toHaveProperty('pageSize');
    expect(body.data.page).toBe(1);
  });

  it('TC-UNI-FE-CONTRACT:角色不足是 HTTP 200 + success:false(不是 403)', () => {
    const fixture = loadBaseline('api_error_insufficient_root');
    expect(fixture.response.status).toBe(200);
    expect(fixture.response.body.success).toBe(false);
  });
});
