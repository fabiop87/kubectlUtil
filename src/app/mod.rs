use crate::command::{self, CommandResult};
use eframe::egui;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::process::Child;
use std::collections::HashMap;

mod mock;
mod ui;

use self::mock::{mock_command_result, mock_scenario_message};

#[derive(PartialEq, Clone, Copy)]
enum Section {
    Connection,
    Overview,
    Cluster,
    Network,
    Aws,
    Parameters,
}

impl Section {
    fn label(self) -> &'static str {
        match self {
            Self::Overview => "Visão geral",
            Self::Connection => "Conexão Kubernetes",
            Self::Cluster => "Cluster",
            Self::Network => "Rede e storage",
            Self::Aws => "AWS & EKS",
            Self::Parameters => "Parameter Store",
        }
    }
}

struct ActivePortForward {
    target: String,
    namespace: String,
    local_port: String,
    remote_port: String,
    child: Option<Child>,
}

#[derive(Clone, Debug)]
struct ParameterEntry {
    name: String,
    kind: String,
    modified: String,
}

pub struct KubectlUtilApp {
    section: Section,
    context: String,
    contexts: Vec<String>,
    context_filter: String,
    namespace: String,
    namespaces: Vec<String>,
    namespace_filter: String,
    pod: String,
    pods: Vec<String>,
    pod_filter: String,
    service: String,
    services: Vec<String>,
    service_filter: String,
    profile: String,
    profiles: Vec<String>,
    profile_filter: String,
    region: String,
    cluster: String,
    parameter_path: String,
    parameter_filter: String,
    parameters: Vec<ParameterEntry>,
    parameter_cache: HashMap<String, Vec<ParameterEntry>>,
    loaded_parameter_key: String,
    pending_parameter_key: String,
    selected_parameter: String,
    revealed_parameter_value: String,
    parameter_value_visible: bool,
    local_port: String,
    remote_port: String,
    filter: String,
    output_title: String,
    output: String,
    status: String,
    busy: bool,
    mock_mode: bool,
    mock_scenario: String,
    explorer_output: String,
    explorer_namespace: String,
    history: Vec<String>,
    port_forwards: Vec<ActivePortForward>,
    tx: Sender<CommandResult>,
    rx: Receiver<CommandResult>,
}

impl Default for KubectlUtilApp {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        let mut app = Self {
            section: Section::Connection,
            context: String::new(),
            contexts: vec![],
            context_filter: String::new(),
            namespace: String::new(),
            namespaces: vec![],
            namespace_filter: String::new(),
            pod: String::new(),
            pods: vec![],
            pod_filter: String::new(),
            service: String::new(),
            services: vec![],
            service_filter: String::new(),
            profile: "default".into(),
            profiles: vec![],
            profile_filter: String::new(),
            region: "us-east-1".into(),
            cluster: String::new(),
            parameter_path: "/".into(),
            parameter_filter: String::new(),
            parameters: Vec::new(),
            parameter_cache: HashMap::new(),
            loaded_parameter_key: String::new(),
            pending_parameter_key: String::new(),
            selected_parameter: String::new(),
            revealed_parameter_value: String::new(),
            parameter_value_visible: false,
            local_port: "5434".into(),
            remote_port: "5432".into(),
            filter: String::new(),
            output_title: "Saída dos comandos".into(),
            output: "Bem-vindo ao kubectlUtil.\n\nSelecione uma seção e execute uma ação para consultar a AWS CLI ou o kubectl.".into(),
            status: "Pronto".into(),
            busy: false,
            mock_mode: false,
            mock_scenario: "Sucesso (dados padrão)".into(),
            explorer_output: String::new(),
            explorer_namespace: String::new(),
            history: Vec::new(),
            port_forwards: Vec::new(),
            tx,
            rx,
        };
        app.refresh_contexts();
        app
    }
}

impl KubectlUtilApp {
    fn launch(&mut self, title: &str, program: &str, args: Vec<String>) {
        if self.busy {
            self.status = "Aguarde a operação atual terminar.".into();
            return;
        }
        self.busy = true;
        self.status = format!("Executando: {title}…");
        self.output_title = title.into();
        self.output = "Consultando…".into();
        let tx = self.tx.clone();
        let title = title.to_string();
        let program = program.to_string();
        let mock_mode = self.mock_mode;
        let mock_scenario = self.mock_scenario.clone();
        thread::spawn(move || {
            let result = if mock_mode {
                mock_command_result(title, &program, &args, &mock_scenario)
            } else {
                command::run(title, &program, &args)
            };
            let _ = tx.send(result);
        });
    }

    fn k(&mut self, title: &str, args: &[&str]) {
        self.launch(title, "kubectl", command::args(args));
    }

    fn aws(&mut self, title: &str, args: Vec<String>) {
        self.launch(title, "aws", args);
    }

    fn perform_login(&mut self) {
        let profile = self.profile.clone();
        // let region = self.region.clone();
        let tx = self.tx.clone();
        let title = "Login AWS SSO".to_string();
        let program = "aws".to_string();

        thread::spawn(move || {
            let args = vec!["sso".into(), "login".into(), "--profile".into(), profile]; //, "--region".into(), region];
            let last_result = command::run(&title, &program, &args);
            let _ = tx.send(last_result);
        });
    }

    fn refresh_contexts(&mut self) {
        // Explicit refresh only: do not poll kubectl in the background.
        self.k("Contextos Kubernetes", &["config", "get-contexts", "-o", "name"]);
    }

    fn load_pod_explorer(&mut self) {
        let ns = self.current_namespace();
        if ns.is_empty() { self.status = "Selecione um namespace.".into(); return; }
        self.explorer_namespace = ns.clone();
        self.launch("Explorador de Pods", "kubectl", vec!["get".into(), "pods".into(), "-n".into(), ns, "-o".into(), "wide".into()]);
    }

    fn run_pod_diagnostics(&mut self) {
        let ns = self.current_namespace();
        if ns.is_empty() { self.status = "Selecione um namespace.".into(); return; }
        // Reuse the last pod listing if available; only query kubectl when no snapshot exists.
        if !self.explorer_output.is_empty() && self.explorer_namespace == ns {
            self.output_title = "Diagnóstico de Pods (cache local)".into();
            self.output = diagnose_pods(&self.explorer_output);
            self.status = "Diagnóstico calculado a partir da última lista carregada; nenhuma nova consulta.".into();
        } else {
            self.explorer_namespace = ns.clone();
            self.launch("Diagnóstico de Pods", "kubectl", vec!["get".into(), "pods".into(), "-n".into(), ns, "-o".into(), "wide".into()]);
        }
    }

    fn start_port_forward(&mut self, target: String) {
        let ns = self.current_namespace();
        let lp = self.local_port.trim().to_string();
        let rp = self.remote_port.trim().to_string();
        if ns.is_empty() || !valid_port(&lp) || !valid_port(&rp) {
            self.status = "Selecione namespace e informe portas válidas (1–65535).".into();
            return;
        }
        if self.port_forwards.iter().any(|pf| pf.local_port == lp) {
            self.status = format!("A porta local {lp} já está sendo usada por outro Port Forward. Escolha outra porta local.");
            return;
        }
        if self.mock_mode {
            if self.mock_scenario != "Sucesso (dados padrão)" {
                self.status = mock_scenario_message(&self.mock_scenario);
                return;
            }
            self.port_forwards.push(ActivePortForward {
                target: target.clone(), namespace: ns.clone(), local_port: lp.clone(), remote_port: rp.clone(), child: None,
            });
            self.status = format!("[MOCK] Port Forward simulado: {target} em {ns}, localhost:{lp} → porta remota {rp}. Nenhuma conexão foi aberta.");
            self.history.push(format!("{} | [MOCK] iniciar port-forward {} {} {}:{}", timestamp(), target, ns, lp, rp));
            return;
        }
        let args = vec!["port-forward".into(), "-n".into(), ns.clone(), target.clone(), format!("{lp}:{rp}")];
        match command::spawn("kubectl", &args) {
            Ok(child) => {
                self.port_forwards.push(ActivePortForward {
                    target: target.clone(), namespace: ns.clone(), local_port: lp.clone(), remote_port: rp.clone(), child: Some(child),
                });
                self.status = format!("Port Forward ativo: {target} em {ns}, localhost:{lp} → porta remota {rp}.");
                self.history.push(format!("{} | iniciar port-forward {} {} {}:{}", timestamp(), target, ns, lp, rp));
            }
            Err(e) => self.status = format!("Falha ao iniciar Port Forward: {e}"),
        }
    }

    fn stop_port_forward(&mut self, index: usize) {
        if index >= self.port_forwards.len() {
            self.status = "Port Forward não encontrado; atualize a lista.".into();
            return;
        }
        let mut pf = self.port_forwards.remove(index);
        if let Some(child) = pf.child.as_mut() {
            match child.kill() {
                Ok(()) => { let _ = child.wait(); self.status = format!("Port Forward encerrado: {} em localhost:{}.", pf.target, pf.local_port); }
                Err(e) => { self.status = format!("Não foi possível encerrar Port Forward {}: {e}", pf.target); return; }
            }
        } else {
            self.status = format!("[MOCK] Port Forward simulado encerrado: {} em localhost:{}.", pf.target, pf.local_port);
        }
        self.history.push(format!("{} | parar port-forward {} {}:{}", timestamp(), pf.target, pf.local_port, pf.remote_port));
    }

    fn refresh_namespaces(&mut self) {
        self.k("Namespaces", &["get", "namespaces", "-o", "custom-columns=:metadata.name"]);
    }

    fn refresh_workloads(&mut self) {
        if self.namespace.trim().is_empty() {
            self.status = "Selecione um namespace primeiro.".into();
            return;
        }
        let ns = self.namespace.clone();
        self.launch("Pods", "kubectl", vec!["get".into(), "pods".into(), "-n".into(), ns.clone(), "-o".into(), "custom-columns=:metadata.name".into()]);
    }

    fn load_services(&mut self) {
        if self.namespace.trim().is_empty() {
            self.status = "Selecione um namespace primeiro.".into();
            return;
        }
        self.k("Services", &["get", "svc", "-n", &self.namespace.clone(), "-o", "custom-columns=:metadata.name"]);
    }

    fn load_profiles(&mut self) {
        self.aws("Perfis AWS", command::args(&["configure", "list-profiles"]));
    }

    fn parameter_cache_key(&self, path: &str) -> String {
        format!("{}|{}|{}", self.profile.trim(), self.region.trim(), path.trim())
    }

    fn load_parameters(&mut self, force_refresh: bool) {
        let profile = self.profile.trim().to_string();
        let region = self.region.trim().to_string();
        let path = self.parameter_path.trim().to_string();
        if profile.is_empty() || region.is_empty() || path.is_empty() {
            self.status = "Informe um perfil, uma região e um nome ou caminho de parâmetro.".into();
            return;
        }
        let key = self.parameter_cache_key(&path);
        if !force_refresh {
            if let Some(cached) = self.parameter_cache.get(&key) {
                self.parameters = cached.clone();
                self.loaded_parameter_key = key.clone();
                self.selected_parameter.clear();
                self.revealed_parameter_value.clear();
                self.parameter_value_visible = false;
                self.status = format!("{} parâmetros carregados do cache local; nenhuma chamada à AWS.", self.parameters.len());
                return;
            }
        }
        // No decryption on listing: only names, types and modification dates are needed.
        // Listagem somente de metadados.
        // describe-parameters não retorna os valores dos parâmetros.
        self.pending_parameter_key = key;

        let args = vec![
            "ssm".into(),
            "describe-parameters".into(),
            "--parameter-filters".into(),
            format!("Key=Name,Option=BeginsWith,Values={path}"),
            "--profile".into(),
            profile,
            "--region".into(),
            region,
            "--output".into(),
            "json".into(),
        ];

        self.launch("Listar parâmetros SSM", "aws", args);
    }

    
    fn clear_parameter_cache(&mut self) {
        self.parameter_cache.clear();
        self.parameters.clear();
        self.loaded_parameter_key.clear();
        self.pending_parameter_key.clear();

        self.selected_parameter.clear();
        self.revealed_parameter_value.clear();
        self.parameter_value_visible = false;

        self.status = "Cache do Parameter Store e dados exibidos em memória foram limpos.".into();
    }

    fn reveal_selected_parameter(&mut self) {
        let current_key = self.parameter_cache_key(self.parameter_path.trim());
        if self.loaded_parameter_key != current_key {
            self.status = "O perfil, a região ou o caminho mudou. Carregue os parâmetros para atualizar a seleção antes de consultar valores.".into();
            return;
        }
        if self.selected_parameter.is_empty() {
            self.status = "Selecione um parâmetro primeiro.".into();
            return;
        }
        let entry = match self.parameters.iter().find(|p| p.name == self.selected_parameter) {
            Some(p) => p,
            None => { self.status = "O parâmetro selecionado não está na lista atual.".into(); return; }
        };
        if entry.kind == "SecureString" {
            self.launch("Revelar SecureString", "aws", vec![
                "ssm".into(), "get-parameter".into(), "--name".into(), entry.name.clone(),
                "--with-decryption".into(), "--profile".into(), self.profile.trim().to_string(),
                "--region".into(), self.region.trim().to_string(), "--query".into(), "Parameter.Value".into(),
                "--output".into(), "text".into()
            ]);
        } else {
            self.launch("Revelar parâmetro", "aws", vec![
                "ssm".into(), "get-parameter".into(), "--name".into(), entry.name.clone(),
                "--profile".into(), self.profile.trim().to_string(), "--region".into(), self.region.trim().to_string(),
                "--query".into(), "Parameter.Value".into(), "--output".into(), "text".into()
            ]);
        }
        self.revealed_parameter_value.clear();
        self.parameter_value_visible = false;
    }

    fn output_action(&mut self, title: &str, args: Vec<String>) {
        self.launch(title, "kubectl", args);
    }

    fn current_namespace(&self) -> String { self.namespace.trim().to_string() }
    fn current_pod(&self) -> String { self.pod.trim().to_string() }
    fn current_service(&self) -> String { self.service.trim().to_string() }

    fn show_output_panel(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.heading(&self.output_title);
            ui.separator();
            ui.label(if self.busy { "Executando…" } else { &self.status });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Limpar").clicked() { self.output.clear(); }
                if ui.button("Copiar").clicked() {
                    ui.ctx().copy_text(self.output.clone());
                    self.status = "Saída copiada.".into();
                }
                if ui.button("Salvar .log").clicked() {
                    match std::fs::write("kubectlUtil-output.log", &self.output) {
                        Ok(_) => self.status = "Salvo em kubectlUtil-output.log".into(),
                        Err(e) => self.status = format!("Erro ao salvar: {e}"),
                    }
                }
            });
        });
        ui.add_space(6.0);
        egui::ScrollArea::vertical()
            .id_salt("output_scroll_area")
            .max_height(f32::INFINITY)
            .stick_to_bottom(true)
            .show(ui, |ui| {
                ui.add_space(4.0);

                ui.add(
                    egui::TextEdit::multiline(&mut self.output)
                        .font(egui::TextStyle::Monospace)
                        .desired_width(f32::INFINITY)
                        .desired_rows(16)
                        .interactive(false),
                );
            });
    }


    fn field_combo(
        ui: &mut egui::Ui,
        label: &str,
        value: &mut String,
        filter: &mut String,
        values: &[String],
        width: f32,
    ) {
        ui.label(label);

        egui::ComboBox::from_id_salt(label)
            .close_behavior(egui::PopupCloseBehavior::IgnoreClicks)
            .selected_text(if value.is_empty() {
                "Selecionar…"
            } else {
                value.as_str()
            })
            .width(width)
            .show_ui(ui, |ui| {
                ui.set_min_width(width);

                // Pesquisa fixa no topo
                ui.add(
                    egui::TextEdit::singleline(filter)
                        .hint_text("Pesquisar / filtrar...")
                        .desired_width((width - 20.0).max(100.0)),
                );

                ui.separator();

                // Apenas a lista de opções terá rolagem
                egui::ScrollArea::vertical()
                    .id_salt(("field_combo_options", label))
                    .max_height(200.0)
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let needle = filter.trim().to_lowercase();
                        let mut found = false;

                        for item in values.iter().filter(|item| {
                            item.to_lowercase().contains(&needle)
                        }) {
                            found = true;

                            if ui
                                .add_sized(
                                    [(width - 20.0).max(100.0), 34.0],
                                    egui::Button::new(item)
                                        .selected(*value == *item),
                                )
                                .clicked()
                            {
                                *value = item.clone();
                                ui.close_menu();
                            }
                        }

                        if !found {
                            ui.label("Nenhum resultado para esse filtro.");
                        }
                    });
            });
    }

    fn nav_button(&mut self, ui: &mut egui::Ui, section: Section) {
        if ui.selectable_label(self.section == section, section.label()).clicked() {
            self.section = section;
        }
    }
}

fn valid_port(port: &str) -> bool {
    port.parse::<u16>().map(|p| p > 0).unwrap_or(false)
}


fn timestamp() -> String {
    // Local, dependency-free timestamp suitable for an audit trail (seconds since Unix epoch).
    match std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        Ok(d) => format!("unix:{}", d.as_secs()),
        Err(_) => "unix:unknown".into(),
    }
}


impl Drop for KubectlUtilApp {
    fn drop(&mut self) {
        for pf in &mut self.port_forwards {
            if let Some(child) = pf.child.as_mut() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}


fn diagnose_pods(source: &str) -> String {
    let mut report = String::from("Diagnóstico rápido (análise local da lista de Pods)\n\n");
    let mut found = false;
    for line in source.lines() {
        let lower = line.to_ascii_lowercase();
        if lower.contains("crashloopbackoff") || lower.contains("imagepullbackoff") || lower.contains("errimagepull") || lower.contains("pending") || lower.contains("error") || lower.contains("notready") {
            report.push_str("[ATENÇÃO] "); report.push_str(line); report.push('\n'); found = true;
        }
    }
    if !found { report.push_str("Nenhum estado suspeito foi identificado na saída. Isso não substitui logs/events.\n"); }
    report.push_str("\nSaída original:\n"); report.push_str(source);
    report
}
