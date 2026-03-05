import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { WarpAccount } from '../types/warp';

interface WarpAccountState {
    accounts: WarpAccount[];
    loading: boolean;
    error: string | null;
    fetchAccounts: () => Promise<void>;
    deleteAccounts: (ids: string[]) => Promise<void>;
    updateAccountTags: (id: string, tags: string[]) => Promise<void>;
}

export const useWarpAccountStore = create<WarpAccountState>((set) => ({
    accounts: [],
    loading: false,
    error: null,

    fetchAccounts: async () => {
        set({ loading: true, error: null });
        try {
            const accounts = await invoke<WarpAccount[]>('get_warp_accounts');
            set({ accounts, loading: false });
        } catch (e: any) {
            set({ error: e.toString(), loading: false });
        }
    },

    deleteAccounts: async (ids: string[]) => {
        try {
            await invoke('delete_warp_accounts', { accountIds: ids });
            set((state) => ({
                accounts: state.accounts.filter((a) => !ids.includes(a.id)),
            }));
        } catch (e: any) {
            console.error('批量删除 Warp 账号失败:', e);
            throw e;
        }
    },

    updateAccountTags: async (id: string, tags: string[]) => {
        try {
            const updated = await invoke<WarpAccount>('update_warp_account_tags', {
                accountId: id,
                tags,
            });
            set((state) => ({
                accounts: state.accounts.map((a) => (a.id === id ? updated : a)),
            }));
        } catch (e: any) {
            console.error('更新标签失败:', e);
            throw e;
        }
    },
}));
