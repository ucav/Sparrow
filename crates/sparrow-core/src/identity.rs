//! Agent identity — the name/role/personality an agent runs under. A trivial
//! foundational type shared by the engine and the memory layer; it lives in
//! `sparrow-core` so neither has to depend on the other just to name an agent.

#[derive(Debug, Clone)]
pub struct Identity {
    pub name: String,
    pub role: String,
    pub personality: String,
}

impl Default for Identity {
    fn default() -> Self {
        Self {
            name: "sparrow".into(),
            role: "software engineer".into(),
            personality: "concise, competent, helpful".into(),
        }
    }
}
