/** API 层通用类型。与后端 `sea_weir_types::dto::common` 对应。 */

export interface ApiResponse<T> {
  success: boolean;
  message: string;
  data?: T;
}

export interface PageInfo<T> {
  items: T[];
  total: number;
  page: number;
  page_size: number;
}

/** 分页请求参数。页码 1 起。 */
export interface PageQuery {
  p: number;
  page_size: number;
}
