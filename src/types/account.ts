export interface Account {
    id: string;
    email: string;
    name?: string;
    token: TokenData;
    device_profile?: DeviceProfile;
    device_history?: DeviceProfileVersion[];
    quota?: QuotaData;
    disabled?: boolean;
    disabled_reason?: string;
    disabled_at?: number;
    proxy_disabled?: boolean;
    proxy_disabled_reason?: string;
    proxy_disabled_at?: number;
    protected_models?: string[];
    custom_label?: string;  // 用户自定义标签
    validation_blocked?: boolean;
    validation_blocked_until?: number;
    validation_blocked_reason?: string;
    validation_url?: string;
    /** 若开启，上游请求体会追加 enabledCreditTypes: ["GOOGLE_ONE_AI"]
     *  使 Google 在免费配额耗尽后继续消耗 AI Credits（仅对 Ultra 家庭组等可用）。*/
    overages_enabled?: boolean;
    created_at: number;
    last_used: number;
}

export interface TokenData {
    access_token: string;
    refresh_token: string;
    expires_in: number;
    expiry_timestamp: number;
    token_type: string;
    email?: string;
}

export interface QuotaData {
    models: ModelQuota[];
    last_updated: number;
    is_forbidden?: boolean;
    forbidden_reason?: string;
    subscription_tier?: string;  // 订阅类型: FREE/PRO/ULTRA
    model_forwarding_rules?: Record<string, string>; // 废弃模型转发表
    /** paid tier 下的 AI Credits 余额列表；free 账号为空数组 */
    ai_credits?: AiCredit[];
}

export interface AiCredit {
    /** credit 种类，如 GOOGLE_ONE_AI / 家庭组共享 */
    credit_type?: string;
    /** 剩余点数 */
    amount: number;
    /** 使用门槛 */
    minimum_amount?: number;
}

export interface ModelQuota {
    name: string;
    percentage: number;
    reset_time: string;
    display_name?: string;
    supports_images?: boolean;
    supports_thinking?: boolean;
    thinking_budget?: number;
    recommended?: boolean;
    max_tokens?: number;
    max_output_tokens?: number;
    supported_mime_types?: Record<string, boolean>;
}

export interface DeviceProfile {
    machine_id: string;
    mac_machine_id: string;
    dev_device_id: string;
    sqm_id: string;
}

export interface DeviceProfileVersion {
    id: string;
    created_at: number;
    label: string;
    profile: DeviceProfile;
    is_current?: boolean;
}

