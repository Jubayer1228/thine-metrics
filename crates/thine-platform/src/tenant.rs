//! Multi-tenant orgs, ingest API keys, and onboarding state.

use chrono::Utc;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Shared demo workspace — seeded metrics/platform data only for this org.
pub const DEMO_ORG_ID: &str = "org_demo";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingState {
    pub step: String,
    #[serde(default)]
    pub completed: Vec<String>,
}

impl Default for OnboardingState {
    fn default() -> Self {
        Self {
            step: "welcome".into(),
            completed: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Organization {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub created_at_ms: i64,
    #[serde(default)]
    pub onboarding: OnboardingState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantUser {
    pub id: Uuid,
    pub email: String,
    pub org_id: String,
    pub roles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingGuide {
    pub org_id: String,
    pub org_name: String,
    pub ingest_api_key: String,
    pub site_url: String,
    pub onboarding: OnboardingState,
    pub steps: Vec<OnboardingStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OnboardingStep {
    pub id: String,
    pub title: String,
    pub detail: String,
    pub done: bool,
}

#[derive(Debug, Default)]
pub struct TenantHub {
    orgs: DashMap<String, Organization>,
    users_by_email: DashMap<String, Uuid>,
    users: DashMap<Uuid, TenantUser>,
    api_keys: DashMap<String, String>,
    oauth_states: DashMap<String, i64>,
}

impl TenantHub {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn ensure_demo_org(&self) {
        if self.orgs.contains_key(DEMO_ORG_ID) {
            return;
        }
        let now = Utc::now().timestamp_millis();
        self.orgs.insert(
            DEMO_ORG_ID.into(),
            Organization {
                id: DEMO_ORG_ID.into(),
                name: "Demo workspace".into(),
                slug: "demo".into(),
                created_at_ms: now,
                onboarding: OnboardingState {
                    step: "done".into(),
                    completed: vec![
                        "welcome".into(),
                        "api_key".into(),
                        "agent".into(),
                        "dashboard".into(),
                    ],
                },
            },
        );
        let _ = self.ensure_ingest_key(DEMO_ORG_ID);
    }

    pub fn org(&self, org_id: &str) -> Option<Organization> {
        self.orgs.get(org_id).map(|e| e.value().clone())
    }

    pub fn resolve_api_key(&self, key: &str) -> Option<String> {
        self.api_keys.get(key).map(|e| e.value().clone())
    }

    pub fn ingest_key_for_org(&self, org_id: &str) -> Option<String> {
        self.api_keys
            .iter()
            .find(|e| e.value().as_str() == org_id)
            .map(|e| e.key().clone())
    }

    fn ensure_ingest_key(&self, org_id: &str) -> String {
        if let Some(k) = self.ingest_key_for_org(org_id) {
            return k;
        }
        let key = format!("thine_ingest_{}", Uuid::new_v4().simple());
        self.api_keys.insert(key.clone(), org_id.into());
        key
    }

    pub fn provision_customer(
        &self,
        email: &str,
        org_name: Option<&str>,
        roles: Vec<String>,
    ) -> (Organization, TenantUser, String) {
        let email = email.trim().to_lowercase();
        if let Some(uid) = self.users_by_email.get(&email) {
            let user = self.users.get(uid.value()).expect("user row");
            let org = self
                .orgs
                .get(&user.org_id)
                .expect("org row")
                .value()
                .clone();
            let key = self.ensure_ingest_key(&org.id);
            return (org, user.clone(), key);
        }

        let now = Utc::now().timestamp_millis();
        let org_id = format!("org_{}", Uuid::new_v4().simple());
        let name = org_name
            .filter(|s| !s.trim().is_empty())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| default_org_name(&email));
        let slug = slugify(&name);
        let org = Organization {
            id: org_id.clone(),
            name: name.clone(),
            slug,
            created_at_ms: now,
            onboarding: OnboardingState::default(),
        };
        self.orgs.insert(org_id.clone(), org.clone());

        let user = TenantUser {
            id: Uuid::new_v4(),
            email: email.clone(),
            org_id: org_id.clone(),
            roles,
        };
        self.users.insert(user.id, user.clone());
        self.users_by_email.insert(email, user.id);

        let key = self.ensure_ingest_key(&org_id);
        (org, user, key)
    }

    pub fn register_oauth_state(&self, state: &str) {
        let exp = Utc::now().timestamp_millis() + 10 * 60 * 1000;
        self.oauth_states.insert(state.into(), exp);
    }

    pub fn consume_oauth_state(&self, state: &str) -> bool {
        let Some(exp) = self.oauth_states.get(state).map(|e| *e.value()) else {
            return false;
        };
        self.oauth_states.remove(state);
        exp >= Utc::now().timestamp_millis()
    }

    pub fn advance_onboarding(&self, org_id: &str, step: &str) -> Option<OnboardingState> {
        let mut entry = self.orgs.get_mut(org_id)?;
        if !entry.onboarding.completed.iter().any(|s| s == step) {
            entry.onboarding.completed.push(step.into());
        }
        entry.onboarding.step = step.into();
        Some(entry.onboarding.clone())
    }

    pub fn onboarding_guide(&self, org_id: &str, site_url: &str) -> Option<OnboardingGuide> {
        let org = self.org(org_id)?;
        let ingest_api_key = self.ensure_ingest_key(org_id);
        let agents_done = false; // filled by caller if needed
        let steps = vec![
            OnboardingStep {
                id: "welcome".into(),
                title: "Create your workspace".into(),
                detail: "Sign in with Google or email to get an isolated org — demo data never mixes with yours.".into(),
                done: org.onboarding.completed.iter().any(|s| s == "welcome"),
            },
            OnboardingStep {
                id: "api_key".into(),
                title: "Copy your ingest API key".into(),
                detail: "Use this key as DD-API-KEY (or THINE-API-KEY) on agents and OTLP clients.".into(),
                done: org.onboarding.completed.iter().any(|s| s == "api_key"),
            },
            OnboardingStep {
                id: "agent".into(),
                title: "Install the Thine Agent".into(),
                detail: "Fleet → Install script embeds your site URL and API key automatically.".into(),
                done: org.onboarding.completed.iter().any(|s| s == "agent") || agents_done,
            },
            OnboardingStep {
                id: "dashboard".into(),
                title: "Open dashboards & monitors".into(),
                detail: "Once metrics arrive, Metrics Explorer and Dashboards show only your org_id tag.".into(),
                done: org.onboarding.completed.iter().any(|s| s == "dashboard"),
            },
        ];
        Some(OnboardingGuide {
            org_id: org.id,
            org_name: org.name,
            ingest_api_key,
            site_url: site_url.into(),
            onboarding: org.onboarding,
            steps,
        })
    }
}

fn default_org_name(email: &str) -> String {
    let domain = email.split('@').nth(1).unwrap_or("workspace");
    let base = domain.split('.').next().unwrap_or(domain);
    format!("{} workspace", capitalize(base))
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

fn slugify(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .trim_matches('-')
        .chars()
        .take(48)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provision_isolated_orgs() {
        let hub = TenantHub::new();
        let (a, _, k1) = hub.provision_customer("a@acme.com", Some("Acme"), vec!["admin".into()]);
        let (b, _, k2) = hub.provision_customer("b@other.io", None, vec!["admin".into()]);
        assert_ne!(a.id, b.id);
        assert_ne!(k1, k2);
        assert_eq!(hub.resolve_api_key(&k1), Some(a.id.clone()));
    }
}
