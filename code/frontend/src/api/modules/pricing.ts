/** `pricing` 域 API。端点清单见 doc/architecture/CONTRACTS.md 附录 A。 */
import { request } from '@/api/client';

/** 模型广场单行。 */
export interface PricingModel {
  model_name: string;
  model_ratio: string;
  completion_ratio: string;
  cache_ratio: string;
  model_price: string | null;
}

export interface PricingView {
  models: PricingModel[];
  group_ratio: Record<string, string>;
  quota_per_unit: number;
}

/** `GET /api/pricing`(公开,可匿名):模型广场。 */
export function getPricing(): Promise<PricingView> {
  return request<PricingView>({ url: '/api/pricing', method: 'get' });
}
