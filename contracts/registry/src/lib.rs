#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

// ── storage ───────────────────────────────────────────────────────────────────

#[contracttype]
enum DataKey {
    Admin,
    Imobiliaria(Address),
}

#[contracttype]
#[derive(Clone)]
pub struct ImobiliariaData {
    pub approved: bool,
    /// running sum of scores across active tenant contracts
    pub score_sum: i128,
    /// number of active tenant contracts
    pub active_count: u32,
}

// ── contract ──────────────────────────────────────────────────────────────────

#[contract]
pub struct Registry;

#[contractimpl]
impl Registry {
    pub fn initialize(e: Env, admin: Address) {
        if e.storage().instance().has(&DataKey::Admin) {
            panic!("already initialized");
        }
        e.storage().instance().set(&DataKey::Admin, &admin);
    }

    /// Approve a new imobiliária partner (admin only).
    pub fn register(e: Env, imobiliaria: Address) {
        require_admin(&e);
        let key = DataKey::Imobiliaria(imobiliaria);
        if e.storage().persistent().has(&key) {
            panic!("already registered");
        }
        e.storage().persistent().set(
            &key,
            &ImobiliariaData {
                approved: true,
                score_sum: 0,
                active_count: 0,
            },
        );
    }

    /// Deactivate a partner without removing their history (admin only).
    pub fn deactivate(e: Env, imobiliaria: Address) {
        require_admin(&e);
        let key = DataKey::Imobiliaria(imobiliaria.clone());
        let mut data: ImobiliariaData = e.storage().persistent().get(&key).expect("not registered");
        data.approved = false;
        e.storage().persistent().set(&key, &data);
    }

    /// Update the score aggregate for an imobiliária when a tenant contract
    /// is created (`count_delta = 1`), updated (`count_delta = 0`), or
    /// closed (`count_delta = -1`). `score_delta` is the signed difference
    /// in the score_sum (new_score - old_score, or just new_score on creation).
    /// Called by the admin / backend after each tenant-contract lifecycle event.
    pub fn update_score(e: Env, imobiliaria: Address, score_delta: i128, count_delta: i32) {
        require_admin(&e);
        let key = DataKey::Imobiliaria(imobiliaria);
        let mut data: ImobiliariaData = e.storage().persistent().get(&key).expect("not registered");

        data.score_sum += score_delta;

        if count_delta > 0 {
            data.active_count += count_delta as u32;
        } else if count_delta < 0 {
            data.active_count = data.active_count.saturating_sub((-count_delta) as u32);
        }

        e.storage().persistent().set(&key, &data);
    }

    /// Average score across all active tenant contracts. Returns 0 if none.
    pub fn get_score(e: Env, imobiliaria: Address) -> i128 {
        let data: ImobiliariaData = e
            .storage()
            .persistent()
            .get(&DataKey::Imobiliaria(imobiliaria))
            .expect("not registered");

        if data.active_count == 0 {
            return 0;
        }
        data.score_sum / data.active_count as i128
    }

    pub fn is_approved(e: Env, imobiliaria: Address) -> bool {
        e.storage()
            .persistent()
            .get::<_, ImobiliariaData>(&DataKey::Imobiliaria(imobiliaria))
            .map(|d| d.approved)
            .unwrap_or(false)
    }

    pub fn admin(e: Env) -> Address {
        e.storage()
            .instance()
            .get(&DataKey::Admin)
            .expect("not initialized")
    }

    pub fn set_admin(e: Env, new_admin: Address) {
        require_admin(&e);
        e.storage().instance().set(&DataKey::Admin, &new_admin);
    }
}

fn require_admin(e: &Env) {
    let admin: Address = e
        .storage()
        .instance()
        .get(&DataKey::Admin)
        .expect("not initialized");
    admin.require_auth();
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::{testutils::Address as _, Env};

    fn setup() -> (Env, Address, RegistryClient<'static>) {
        let e = Env::default();
        e.mock_all_auths();
        let admin = Address::generate(&e);
        let contract_id = e.register(Registry, ());
        let client = RegistryClient::new(&e, &contract_id);
        client.initialize(&admin);
        (e, admin, client)
    }

    #[test]
    fn register_and_score() {
        let (e, _admin, client) = setup();
        let imob = Address::generate(&e);

        client.register(&imob);
        assert!(client.is_approved(&imob));
        assert_eq!(client.get_score(&imob), 0);

        // tenant contract created with score 80
        client.update_score(&imob, &80, &1);
        assert_eq!(client.get_score(&imob), 80);

        // second tenant contract with score 60
        client.update_score(&imob, &60, &1);
        assert_eq!(client.get_score(&imob), 70); // (80+60)/2

        // first tenant contract closed (remove score 80)
        client.update_score(&imob, &-80, &-1);
        assert_eq!(client.get_score(&imob), 60); // only 60 active
    }

    #[test]
    fn deactivate() {
        let (e, _admin, client) = setup();
        let imob = Address::generate(&e);
        client.register(&imob);
        client.deactivate(&imob);
        assert!(!client.is_approved(&imob));
    }
}
