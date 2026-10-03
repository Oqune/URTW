use crate::model::Service;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CoreInfo {
    pub id: String,
    pub name: String,
    pub author: String,
    pub source: String,
    pub license: String,
    pub executable: String,
    pub format: String,
    pub default_port: u16,
    pub tun: bool,
    pub process_rules: bool,
    #[serde(default)]
    pub generator: String,
}
#[derive(Debug, Clone, Default)]
pub struct CoreState {
    pub service: Service,
    pub running_config: Option<PathBuf>,
    pub running_port: Option<u16>,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub core: String,
    pub path: PathBuf,
    pub port: u16,
    pub validation: String,
    #[serde(default)]
    pub generated: bool,
}
#[derive(Debug, Clone, Deserialize)]
pub struct Endpoint {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub server: String,
    pub port: u16,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Rule {
    pub kind: String,
    pub value: String,
    pub action: String,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Standard {
    pub id: String,
    pub name: String,
    #[serde(rename = "default")]
    pub fallback: String,
    pub browser_domains: Vec<String>,
    pub rules: Vec<Rule>,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CoreOptions {
    pub port: u16,
    pub dns: Vec<String>,
    pub dns_strategy: String,
    pub ipv6: bool,
    pub tun: bool,
    pub log_level: String,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Workspace {
    pub schema_version: u32,
    pub active_core: String,
    pub active_endpoint: Option<String>,
    pub active_standard: String,
    pub selected_profiles: BTreeMap<String, String>,
    pub profiles: Vec<Profile>,
    pub endpoints: Vec<Endpoint>,
    pub standards: Vec<Standard>,
    pub core_options: BTreeMap<String, CoreOptions>,
}
impl Default for Workspace {
    fn default() -> Self {
        Self {
            schema_version: 1,
            active_core: "mihomo".into(),
            active_endpoint: None,
            active_standard: "selective".into(),
            selected_profiles: BTreeMap::new(),
            profiles: Vec::new(),
            endpoints: Vec::new(),
            standards: Vec::new(),
            core_options: BTreeMap::new(),
        }
    }
}
impl Workspace {
    pub fn core_profiles(&self) -> Vec<&Profile> {
        self.profiles
            .iter()
            .filter(|p| p.core == self.active_core)
            .collect()
    }
    pub fn selected_profile(&self) -> Option<&Profile> {
        let id = self.selected_profiles.get(&self.active_core)?;
        self.profiles
            .iter()
            .find(|p| p.id == *id && p.core == self.active_core)
    }
    pub fn standard(&self) -> Option<&Standard> {
        self.standards.iter().find(|s| s.id == self.active_standard)
    }
    pub fn endpoint(&self) -> Option<&Endpoint> {
        let id = self.active_endpoint.as_ref()?;
        self.endpoints.iter().find(|e| e.id == *id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_endpoint_credentials_are_not_part_of_ui_model() {
        let endpoint: Endpoint = serde_json::from_str(r#"{"id":"test","name":"Example","type":"vless","server":"example.com","port":443,"data":{"password":"private-test-value"}}"#).unwrap();
        assert!(!format!("{endpoint:?}").contains("private-test-value"));
    }
    #[test]
    fn selection_is_scoped_to_each_core() {
        let mut workspace = Workspace::default();
        for core in ["mihomo", "singbox"] {
            workspace.profiles.push(Profile {
                id: core.into(),
                name: core.into(),
                core: core.into(),
                path: "config".into(),
                port: 17890,
                validation: "pending".into(),
                generated: false,
            });
            workspace.selected_profiles.insert(core.into(), core.into());
        }
        assert_eq!(workspace.selected_profile().unwrap().core, "mihomo");
        workspace.active_core = "singbox".into();
        assert_eq!(workspace.selected_profile().unwrap().core, "singbox");
        assert_eq!(workspace.core_profiles().len(), 1);
    }
}
