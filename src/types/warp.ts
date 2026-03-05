/** Warp 账号数据（后端原样返回的结构） */
export interface WarpAccount {
    id: string;
    email: string;
    user_id?: string | null;
    tags?: string[] | null;

    // 凭据字段
    auth_token: string;
    refresh_token?: string | null;
    device_id?: string | null;
    expires_at?: number | null;

    // 配额/计划状态
    plan_type?: string | null;
    quota_status?: Record<string, unknown> | null;

    // 存储元数据
    created_at: number;
    last_used: number;
}
