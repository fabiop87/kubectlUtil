use super::{KubectlUtilApp, Selectable};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, io, path::PathBuf};

#[derive(Clone, Deserialize, Serialize)]
struct Favorite {
    context: String,
    namespace: String,
    service: String,
    #[serde(default)]
    pod: String,
    #[serde(default)]
    local_port: Option<String>,
}

type Entries = BTreeMap<String, Favorite>;

pub(super) struct Favorites {
    file: Result<PathBuf, String>,
    entries: Entries,
    pub(super) selected: Selectable,
    pub(super) name: String,
}

impl Favorites {
    pub(super) fn load() -> Self {
        let mut entries = Entries::new();
        let file = std::env::var_os("APPDATA")
            .filter(|dir| !dir.is_empty())
            .ok_or_else(|| "APPDATA indisponível.".to_string())
            .map(|dir| PathBuf::from(dir).join("kubectlUtil").join("config.json"))
            .and_then(|file| {
                match fs::read_to_string(&file) {
                    Ok(json) => {
                        entries = serde_json::from_str(&json)
                            .map_err(|e| format!("config.json inválido: {e}"))?;
                    }
                    Err(e) if e.kind() == io::ErrorKind::NotFound => {}
                    Err(e) => return Err(format!("Falha ao ler config.json: {e}")),
                }
                Ok(file)
            });
        let selected = Selectable {
            items: entries.keys().cloned().collect(),
            ..Default::default()
        };
        Self {
            file,
            entries,
            selected,
            name: String::new(),
        }
    }

    pub(super) fn error(&self) -> Option<&str> {
        self.file.as_ref().err().map(String::as_str)
    }

    fn persist(&mut self, entries: Entries) -> Result<(), String> {
        let path = self.file.as_ref().map_err(Clone::clone)?;
        let dir = path.parent().expect("config.json tem diretório");
        fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        let mut temp = tempfile::NamedTempFile::new_in(dir).map_err(|e| e.to_string())?;
        serde_json::to_writer_pretty(&mut temp, &entries).map_err(|e| e.to_string())?;
        temp.as_file().sync_all().map_err(|e| e.to_string())?;
        temp.persist(path).map_err(|e| e.to_string())?;
        self.selected.items = entries.keys().cloned().collect();
        self.entries = entries;
        Ok(())
    }
}

impl KubectlUtilApp {
    pub(super) fn apply_favorite(&mut self, name: String) {
        if let Some(favorite) = self.favorites.entries.get(&name).cloned() {
            if self.context.value != favorite.context {
                self.context.value = favorite.context;
                self.on_context_changed();
            }
            self.set_namespace(favorite.namespace);
            self.service.value = favorite.service;
            self.pod.value = favorite.pod;
            if let Some(port) = favorite.local_port {
                self.local_port = port;
            }
            self.favorites.name = name.clone();
            self.favorites.selected.value = name;
        }
    }

    pub(super) fn save_favorite(&mut self, edit: bool) {
        let selected = &self.favorites.selected.value;
        let name = self.favorites.name.trim().to_owned();
        let favorite = Favorite {
            context: self.context.trimmed().into(),
            namespace: self.namespace.trimmed().into(),
            service: self.service.trimmed().into(),
            pod: self.pod.trimmed().into(),
            local_port: Some(self.local_port.trim().into()),
        };
        if name.is_empty()
            || favorite.context.is_empty()
            || favorite.namespace.is_empty()
            || (favorite.service.is_empty() && favorite.pod.is_empty())
        {
            self.status =
                "Preencha nome, contexto, namespace e pelo menos um Pod ou Service.".into();
            return;
        }
        let mut entries = self.favorites.entries.clone();
        if edit && entries.remove(selected).is_none() {
            self.status = "Selecione um favorito para editar.".into();
            return;
        }
        if entries.contains_key(&name) {
            self.status = "Já existe um favorito com esse nome.".into();
            return;
        }
        entries.insert(name.clone(), favorite);
        self.status = match self.favorites.persist(entries) {
            Ok(()) => {
                self.favorites.selected.value = name.clone();
                self.favorites.name = name;
                "Favorito salvo.".into()
            }
            Err(e) => format!("Falha ao salvar favorito: {e}"),
        };
    }

    pub(super) fn delete_favorite(&mut self) {
        let mut entries = self.favorites.entries.clone();
        if entries.remove(&self.favorites.selected.value).is_none() {
            self.status = "Selecione um favorito para excluir.".into();
            return;
        }
        self.status = match self.favorites.persist(entries) {
            Ok(()) => {
                self.favorites.selected.value.clear();
                self.favorites.name.clear();
                "Favorito excluído.".into()
            }
            Err(e) => format!("Falha ao excluir favorito: {e}"),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn favorites_can_be_saved_applied_renamed_and_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut app = KubectlUtilApp::new();
        app.favorites = Favorites {
            file: Ok(path.clone()),
            entries: Entries::new(),
            selected: Selectable::default(),
            name: "integrador-prod".into(),
        };
        app.context.value = "c".into();
        app.namespace.value = "n".into();
        app.service.value = "s".into();
        app.pod.value = "pod-salvo".into();
        app.local_port = "15432".into();
        app.save_favorite(false);
        app.favorites.entries = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();

        app.context.value = "outro".into();
        app.pod.value = "pod-antigo".into();
        app.local_port = "5434".into();
        app.apply_favorite("integrador-prod".into());
        assert_eq!(app.context.value, "c");
        assert_eq!(app.namespace.value, "n");
        assert_eq!(app.service.value, "s");
        assert_eq!(app.pod.value, "pod-salvo");
        assert_eq!(app.local_port, "15432");

        app.favorites.name = "integrador".into();
        app.service.value = "novo-service".into();
        app.save_favorite(true);
        let saved: Entries = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved["integrador"].service, "novo-service");

        app.delete_favorite();
        let saved: Entries = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert!(saved.is_empty());
    }

    #[test]
    fn favorites_saved_without_pod_or_local_port_still_load() {
        let saved: Entries = serde_json::from_str(
            r#"{"integrador-prod":{"context":"c","namespace":"n","service":"s"}}"#,
        )
        .unwrap();
        assert!(saved["integrador-prod"].pod.is_empty());
        assert!(saved["integrador-prod"].local_port.is_none());
    }
}
