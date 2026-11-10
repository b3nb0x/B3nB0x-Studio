use serde::{Deserialize, Serialize};
use std::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RuleEffect {
    Allow,
    Deny,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRule {
    pub effect: RuleEffect,
    pub action: String,
    #[serde(default)]
    pub scope: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionCheck {
    pub action: String,
    pub scope: Option<String>,
    pub allowed: bool,
    pub matched_rule: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionConfig {
    #[serde(default = "default_mode")]
    pub mode: String,
    #[serde(default)]
    pub rules: Vec<PermissionRule>,
}

impl Default for PermissionConfig {
    fn default() -> Self {
        Self {
            mode: default_mode(),
            rules: vec![PermissionRule {
                effect: RuleEffect::Allow,
                action: "*".into(),
                scope: None,
            }],
        }
    }
}

fn default_mode() -> String {
    "default-allow".into()
}

pub struct Permissions {
    mode: String,
    rules: RwLock<Vec<PermissionRule>>,
}

impl Permissions {
    pub fn new(config: &PermissionConfig) -> Self {
        Self {
            mode: config.mode.clone(),
            rules: RwLock::new(config.rules.clone()),
        }
    }

    pub fn mode(&self) -> &str {
        &self.mode
    }

    pub fn rules(&self) -> Vec<PermissionRule> {
        self.rules.read().unwrap().clone()
    }

    pub fn add_rule(&self, rule: PermissionRule) {
        self.rules.write().unwrap().push(rule);
    }

    pub fn check(&self, action: &str, scope: Option<&str>) -> PermissionCheck {
        let rules = self.rules.read().unwrap();

        let mut matched: Option<&PermissionRule> = None;
        for rule in rules.iter().rev() {
            let action_match = rule.action == "*" || rule.action == action;
            let scope_match = match (&rule.scope, scope) {
                (None, _) => true,
                (Some(rs), Some(s)) => rs == "*" || rs == s,
                (Some(_), None) => false,
            };
            if action_match && scope_match {
                matched = Some(rule);
                break;
            }
        }

        let allowed = match matched {
            Some(rule) => rule.effect == RuleEffect::Allow,
            None => self.mode == "default-allow",
        };

        PermissionCheck {
            action: action.to_string(),
            scope: scope.map(|s| s.to_string()),
            allowed,
            matched_rule: matched.map(|r| format!("{:?} {}", r.effect, r.action)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn permissions(rules: Vec<PermissionRule>, mode: &str) -> Permissions {
        Permissions::new(&PermissionConfig {
            mode: mode.into(),
            rules,
        })
    }

    #[test]
    fn default_allow_mode() {
        let p = permissions(vec![], "default-allow");
        assert!(p.check("chat", None).allowed);
        assert!(p.check("anything", None).allowed);
    }

    #[test]
    fn default_deny_with_explicit_allow() {
        let p = permissions(
            vec![PermissionRule {
                effect: RuleEffect::Allow,
                action: "chat".into(),
                scope: None,
            }],
            "default-deny",
        );
        assert!(p.check("chat", None).allowed);
        assert!(!p.check("pipeline".into(), None).allowed);
    }

    #[test]
    fn deny_overrides_allow_by_order() {
        let p = permissions(
            vec![
                PermissionRule {
                    effect: RuleEffect::Allow,
                    action: "*".into(),
                    scope: None,
                },
                PermissionRule {
                    effect: RuleEffect::Deny,
                    action: "reboot".into(),
                    scope: None,
                },
            ],
            "default-allow",
        );
        assert!(p.check("chat", None).allowed);
        assert!(!p.check("reboot", None).allowed);
        assert_eq!(
            p.check("reboot", None).matched_rule,
            Some("Deny reboot".into())
        );
    }

    #[test]
    fn scope_matching() {
        let p = permissions(
            vec![PermissionRule {
                effect: RuleEffect::Allow,
                action: "script".into(),
                scope: Some("scripts".into()),
            }],
            "default-deny",
        );
        assert!(p.check("script", Some("scripts")).allowed);
        assert!(!p.check("script", Some("/etc")).allowed);
        assert!(!p.check("script", None).allowed);
    }
}
