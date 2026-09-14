/**
 * 控制台-看板。展示用量概览。
 *
 * - 所有登录用户:本人总消费额度、最近 60s rpm/tpm(/api/log/self/stat)
 * - admin 及以上:额外展示用户/渠道/令牌数量与全局统计(/api/data/)
 *
 * 待补:按模型/渠道的分布、分时段趋势图。
 */
import { useEffect, useState } from 'react';
import { Card, Col, Row, Statistic, Typography } from 'antd';

import { api } from '@/api';
import { useUserStore } from '@/stores/user';
import { ROLE, QUOTA_PER_UNIT } from '@/types';

function quotaToUsd(quota: number): string {
  return `$${(quota / QUOTA_PER_UNIT).toFixed(4)}`;
}

export default function Dashboard() {
  const role = useUserStore((s) => s.role);
  const isAdmin = role >= ROLE.ADMIN;

  const [loading, setLoading] = useState(true);
  const [selfQuota, setSelfQuota] = useState(0);
  const [rpm, setRpm] = useState(0);
  const [tpm, setTpm] = useState(0);
  const [counts, setCounts] = useState({ users: 0, channels: 0, tokens: 0 });
  const [globalQuota, setGlobalQuota] = useState(0);

  useEffect(() => {
    let alive = true;
    setLoading(true);
    const self = api.dashboard.getSelfStat().then((s) => {
      if (!alive) return;
      setSelfQuota(s.total_quota);
      setRpm(s.rpm);
      setTpm(s.tpm);
    });
    const overview = isAdmin
      ? api.dashboard.getOverview().then((o) => {
          if (!alive) return;
          setCounts({ users: o.user_count, channels: o.channel_count, tokens: o.token_count });
          setGlobalQuota(o.total_quota);
          setRpm(o.rpm);
          setTpm(o.tpm);
        })
      : Promise.resolve();

    void Promise.allSettled([self, overview]).finally(() => {
      if (alive) setLoading(false);
    });
    return () => {
      alive = false;
    };
  }, [isAdmin]);

  return (
    <Row gutter={[16, 16]}>
      <Col span={24}>
        <Typography.Title level={4} style={{ marginTop: 0 }}>
          用量概览
        </Typography.Title>
      </Col>

      {isAdmin && (
        <>
          <Col xs={24} sm={12} md={8}>
            <Card loading={loading}>
              <Statistic title="全局总消费" value={quotaToUsd(globalQuota)} />
            </Card>
          </Col>
          <Col xs={24} sm={12} md={8}>
            <Card loading={loading}>
              <Statistic title="用户数" value={counts.users} />
            </Card>
          </Col>
          <Col xs={24} sm={12} md={8}>
            <Card loading={loading}>
              <Statistic title="渠道数" value={counts.channels} />
            </Card>
          </Col>
          <Col xs={24} sm={12} md={8}>
            <Card loading={loading}>
              <Statistic title="令牌数" value={counts.tokens} />
            </Card>
          </Col>
        </>
      )}

      <Col xs={24} sm={12} md={8}>
        <Card loading={loading}>
          <Statistic title="我的消费" value={quotaToUsd(selfQuota)} />
        </Card>
      </Col>
      <Col xs={24} sm={12} md={8}>
        <Card loading={loading}>
          <Statistic title="最近 60s 请求数 (rpm)" value={rpm} />
        </Card>
      </Col>
      <Col xs={24} sm={12} md={8}>
        <Card loading={loading}>
          <Statistic title="最近 60s Token 数 (tpm)" value={tpm} />
        </Card>
      </Col>
    </Row>
  );
}
