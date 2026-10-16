//! AWS Systems Manager Parameter Store: estado em memória, cache e montagem de comandos.

use serde_json::Value;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParameterEntry {
    pub name: String,
    pub kind: String,
    pub modified: String,
}

pub struct ParameterStore {
    pub path: String,
    pub filter: String,
    pub entries: Vec<ParameterEntry>,
    pub cache: HashMap<String, Vec<ParameterEntry>>,
    /// Chave (perfil|região|caminho) da lista exibida agora.
    pub loaded_key: String,
    /// Chave da consulta em andamento.
    pub pending_key: String,
    pub selected: String,
    /// Valor consultado sob demanda (nunca vai ao histórico nem ao painel geral).
    pub revealed: String,
    pub revealed_visible: bool,
}

impl ParameterStore {
    pub fn new() -> Self {
        Self {
            path: "/".into(),
            filter: String::new(),
            entries: Vec::new(),
            cache: HashMap::new(),
            loaded_key: String::new(),
            pending_key: String::new(),
            selected: String::new(),
            revealed: String::new(),
            revealed_visible: false,
        }
    }

    /// Esquece seleção e valor consultado.
    pub fn reset_selection(&mut self) {
        self.selected.clear();
        self.hide_value();
    }

    pub fn select(&mut self, name: String) {
        self.selected = name;
        self.hide_value();
    }

    /// Oculta e remove o valor do estado; Rust não garante zerar a memória alocada.
    pub fn hide_value(&mut self) {
        self.revealed.clear();
        self.revealed_visible = false;
    }

    pub fn set_revealed(&mut self, value: String) {
        self.revealed = value;
        self.revealed_visible = true;
    }

    /// Mostra uma lista já conhecida (cache) e zera a seleção.
    pub fn show(&mut self, key: String, entries: Vec<ParameterEntry>) {
        self.entries = entries;
        self.loaded_key = key;
        self.reset_selection();
    }

    /// Guarda no cache o resultado da consulta pendente e o exibe.
    pub fn store_loaded(&mut self, entries: Vec<ParameterEntry>) {
        let key = self.pending_key.clone();
        self.cache.insert(key.clone(), entries.clone());
        self.show(key, entries);
    }

    pub fn clear_all(&mut self) {
        self.cache.clear();
        self.entries.clear();
        self.loaded_key.clear();
        self.pending_key.clear();
        self.reset_selection();
    }
}

pub fn cache_key(profile: &str, region: &str, path: &str) -> String {
    format!("{}|{}|{}", profile.trim(), region.trim(), path.trim())
}

/// `describe-parameters` retorna só metadados (nunca os valores).
pub fn describe_args(profile: &str, region: &str, path: &str) -> Vec<String> {
    vec![
        "ssm".into(),
        "describe-parameters".into(),
        "--parameter-filters".into(),
        format!("Key=Name,Option=BeginsWith,Values={path}"),
        "--profile".into(),
        profile.into(),
        "--region".into(),
        region.into(),
        "--output".into(),
        "json".into(),
    ]
}

/// `get-parameter` de um único valor; `decrypt` só para `SecureString`.
pub fn get_args(name: &str, decrypt: bool, profile: &str, region: &str) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "ssm".into(),
        "get-parameter".into(),
        "--name".into(),
        name.into(),
    ];
    if decrypt {
        args.push("--with-decryption".into());
    }
    args.extend(
        [
            "--profile",
            profile,
            "--region",
            region,
            "--query",
            "Parameter.Value",
            "--output",
            "text",
        ]
        .map(String::from),
    );
    args
}

/// Interpreta a saída JSON de `aws ssm describe-parameters`.
pub fn parse_parameters(json: &str) -> Result<Vec<ParameterEntry>, String> {
    let value: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    Ok(value["Parameters"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|p| ParameterEntry {
            name: p["Name"].as_str().unwrap_or("").to_string(),
            kind: p["Type"].as_str().unwrap_or("Unknown").to_string(),
            modified: p["LastModifiedDate"].as_str().unwrap_or("—").to_string(),
        })
        .filter(|p| !p.name.is_empty())
        .collect())
}

/// Erros típicos de credenciais AWS expiradas ou inválidas.
pub fn is_invalid_credentials(output: &str) -> bool {
    output.contains("UnrecognizedClientException")
        || output
            .to_lowercase()
            .contains("security token included in the request is invalid")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str) -> ParameterEntry {
        ParameterEntry {
            name: name.into(),
            kind: "String".into(),
            modified: "—".into(),
        }
    }

    #[test]
    fn parse_parameters_reads_metadata() {
        let json = r#"{"Parameters":[{"Name":"/dev/app/API_URL","Type":"String","LastModifiedDate":"2026-10-01T12:00:00Z"},{"Name":"/dev/app/DB_PASSWORD","Type":"SecureString"}]}"#;
        let entries = parse_parameters(json).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "/dev/app/API_URL");
        assert_eq!(entries[0].modified, "2026-10-01T12:00:00Z");
        assert_eq!(entries[1].kind, "SecureString");
        assert_eq!(entries[1].modified, "—");
    }

    #[test]
    fn parse_parameters_handles_empty_and_invalid_input() {
        assert!(parse_parameters(r#"{"Parameters":[]}"#).unwrap().is_empty());
        assert!(parse_parameters("{}").unwrap().is_empty());
        assert!(parse_parameters("isto não é json").is_err());
    }

    #[test]
    fn parse_parameters_skips_entries_without_name() {
        let entries =
            parse_parameters(r#"{"Parameters":[{"Type":"String"},{"Name":"/a"}]}"#).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "/a");
    }

    #[test]
    fn cache_key_trims_each_part() {
        assert_eq!(
            cache_key(" dev ", " sa-east-1", "/app/ "),
            "dev|sa-east-1|/app/"
        );
    }

    #[test]
    fn describe_args_filter_by_prefix() {
        let args = describe_args("dev", "sa-east-1", "/app/");
        assert_eq!(args[3], "Key=Name,Option=BeginsWith,Values=/app/");
        assert!(
            args.windows(2)
                .any(|w| w[0] == "--profile" && w[1] == "dev")
        );
        assert!(
            args.windows(2)
                .any(|w| w[0] == "--region" && w[1] == "sa-east-1")
        );
        assert!(!args.iter().any(|a| a == "--with-decryption"));
    }

    #[test]
    fn get_args_decrypts_only_when_requested() {
        let plain = get_args("/a", false, "dev", "sa-east-1");
        assert!(!plain.iter().any(|a| a == "--with-decryption"));
        assert_eq!(plain.last().map(String::as_str), Some("text"));

        let secure = get_args("/a", true, "dev", "sa-east-1");
        assert!(secure.iter().any(|a| a == "--with-decryption"));
        assert_eq!(secure[..4], ["ssm", "get-parameter", "--name", "/a"]);
    }

    #[test]
    fn detects_invalid_credentials() {
        assert!(is_invalid_credentials(
            "An error occurred (UnrecognizedClientException) when calling"
        ));
        assert!(is_invalid_credentials(
            "The security token included in the request is invalid."
        ));
        assert!(!is_invalid_credentials("AccessDeniedException"));
    }

    #[test]
    fn hide_value_erases_secret() {
        let mut store = ParameterStore::new();
        store.set_revealed("segredo".into());
        assert!(store.revealed_visible);
        store.hide_value();
        assert!(store.revealed.is_empty());
        assert!(!store.revealed_visible);
    }

    #[test]
    fn store_loaded_caches_by_pending_key_and_resets_selection() {
        let mut store = ParameterStore::new();
        store.selected = "/antigo".into();
        store.set_revealed("x".into());
        store.pending_key = "dev|sa-east-1|/".into();
        store.store_loaded(vec![entry("/a"), entry("/b")]);

        assert_eq!(store.entries.len(), 2);
        assert_eq!(store.loaded_key, "dev|sa-east-1|/");
        assert_eq!(store.cache["dev|sa-east-1|/"].len(), 2);
        assert!(store.selected.is_empty());
        assert!(store.revealed.is_empty());
    }

    #[test]
    fn clear_all_wipes_cache_and_state() {
        let mut store = ParameterStore::new();
        store.pending_key = "k".into();
        store.store_loaded(vec![entry("/a")]);
        store.clear_all();
        assert!(store.cache.is_empty());
        assert!(store.entries.is_empty());
        assert!(store.loaded_key.is_empty());
    }
}
