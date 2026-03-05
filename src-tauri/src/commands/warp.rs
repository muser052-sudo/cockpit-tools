use crate::models::warp::WarpAccount;
use crate::modules::warp_account;
use tauri::command;

#[command]
pub fn get_warp_accounts() -> Result<Vec<WarpAccount>, String> {
    Ok(warp_account::list_accounts())
}

#[command]
pub fn delete_warp_accounts(account_ids: Vec<String>) -> Result<(), String> {
    warp_account::delete_accounts(&account_ids)
}

#[command]
pub fn update_warp_account_tags(
    account_id: String,
    tags: Vec<String>,
) -> Result<WarpAccount, String> {
    warp_account::update_tags(&account_id, tags)
}
