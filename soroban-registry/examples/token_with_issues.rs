//! WARNING: This example intentionally demonstrates an anti-pattern.
//! Hardcoded string storage keys can collide silently and corrupt state.
//! See `examples/token_fixed.rs` for the recommended typed-key approach.

use soroban_sdk::{contract, contractimpl, Address, Env, Symbol};

const STORAGE_KEY_BALANCE: &str = "balance";
const STORAGE_KEY_ALLOWANCE: &str = "balance"; // Intentional collision with `STORAGE_KEY_BALANCE`.

#[contract]
pub struct TokenWithIssues;

#[contractimpl]
impl TokenWithIssues {
    /// WARNING: anti-pattern for demonstration only.
    /// This writes to a global hardcoded key and does not namespace by account.
    pub fn set_balance(env: Env, owner: Address, amount: i128) {
        owner.require_auth();
        env.storage()
            .persistent()
            .set(&Symbol::new(&env, STORAGE_KEY_BALANCE), &amount);
    }

    /// WARNING: anti-pattern for demonstration only.
    /// This uses a different logical concept but the same raw key value as `set_balance`.
    pub fn set_allowance(env: Env, owner: Address, spender: Address, amount: i128) {
        owner.require_auth();
        let _ = spender;
        env.storage()
            .persistent()
            .set(&Symbol::new(&env, STORAGE_KEY_ALLOWANCE), &amount);
    }

    pub fn get_balance(env: Env) -> i128 {
        env.storage()
            .persistent()
            .get::<_, i128>(&Symbol::new(&env, STORAGE_KEY_BALANCE))
            .unwrap_or(0)
    }

    pub fn get_allowance(env: Env) -> i128 {
        env.storage()
            .persistent()
            .get::<_, i128>(&Symbol::new(&env, STORAGE_KEY_ALLOWANCE))
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::TokenWithIssues;
    use soroban_sdk::{testutils::Address as _, Address, Env};

    #[test]
    fn hardcoded_keys_collide_and_overwrite_balance() {
        let env = Env::default();
        env.mock_all_auths();

        let owner = Address::generate(&env);
        let spender = Address::generate(&env);

        TokenWithIssues::set_balance(env.clone(), owner.clone(), 100);
        assert_eq!(TokenWithIssues::get_balance(env.clone()), 100);

        // Writing allowance overwrites balance because both map to "balance".
        TokenWithIssues::set_allowance(env.clone(), owner, spender, 7);

        assert_eq!(TokenWithIssues::get_allowance(env.clone()), 7);
        assert_eq!(TokenWithIssues::get_balance(env), 7);
    }

    #[test]
    fn hardcoded_keys_collide_and_overwrite_allowance() {
        let env = Env::default();
        env.mock_all_auths();

        let owner = Address::generate(&env);

        TokenWithIssues::set_allowance(env.clone(), owner.clone(), owner.clone(), 55);
        assert_eq!(TokenWithIssues::get_allowance(env.clone()), 55);

        // Writing balance now overwrites what allowance previously stored.
        TokenWithIssues::set_balance(env.clone(), owner, 12);

        assert_eq!(TokenWithIssues::get_balance(env.clone()), 12);
        assert_eq!(TokenWithIssues::get_allowance(env), 12);
    }
}
