import { useEffect, useState } from 'react';
import { Coins } from 'lucide-react';
import { useTranslation } from 'react-i18next';
import { Account } from '../../types/account';
import { cn } from '../../utils/cn';
import { toggleOveragesEnabled } from '../../services/accountService';
import { useAccountStore } from '../../stores/useAccountStore';

interface OverageToggleIconButtonProps {
    account: Account;
}

/**
 * 列表/卡片里的 AI Credits 溢出开关图标按钮。
 *
 * 乐观态说明：切换时先乐观更新 UI，等 toggleOveragesEnabled + fetchAccounts 回来后，
 * 用 store 的真值接管；toggle 本身失败就回滚，refresh 失败就保留乐观态避免抖动。
 */
export default function OverageToggleIconButton({ account }: OverageToggleIconButtonProps) {
    const { t } = useTranslation();
    const fetchAccounts = useAccountStore((s) => s.fetchAccounts);

    const [busy, setBusy] = useState(false);
    const [optimistic, setOptimistic] = useState<boolean | null>(null);

    useEffect(() => {
        setOptimistic(null);
        setBusy(false);
    }, [account.id]);

    const on = optimistic ?? (account.overages_enabled ?? false);

    const handleClick = async (e: React.MouseEvent) => {
        e.stopPropagation();
        if (busy) return;

        const next = !on;
        const prev = on;
        setBusy(true);
        setOptimistic(next);

        try {
            try {
                await toggleOveragesEnabled(account.id, next);
            } catch (err) {
                setOptimistic(prev);
                console.error('toggle overages failed:', err);
                return;
            }
            try {
                await fetchAccounts();
                setOptimistic(null);
            } catch (err) {
                console.warn('toggle overages succeeded but refresh failed:', err);
            }
        } finally {
            setBusy(false);
        }
    };

    const title = t(
        on ? 'accounts.overages.disable_tooltip' : 'accounts.overages.enable_tooltip',
        on
            ? '关闭 AI Credits 溢出（不再使用 Credits 兜底）'
            : '开启 AI Credits 溢出（免费配额耗尽后继续消耗 Credits）'
    );

    return (
        <button
            className={cn(
                'p-1.5 rounded-lg transition-all',
                on
                    ? 'text-purple-600 bg-purple-50 hover:bg-purple-100 dark:text-purple-300 dark:bg-purple-900/30 dark:hover:bg-purple-900/50'
                    : 'text-gray-400 hover:text-purple-600 hover:bg-purple-50 dark:text-gray-500 dark:hover:text-purple-400 dark:hover:bg-purple-900/30',
                busy && 'opacity-60 cursor-not-allowed'
            )}
            onClick={handleClick}
            title={title}
            disabled={busy}
            aria-pressed={on}
        >
            <Coins className={cn('w-3.5 h-3.5', busy && 'animate-pulse')} />
        </button>
    );
}
