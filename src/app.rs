use crate::command::{self, CommandResult};
use eframe::egui;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::process::Child;
use std::collections::HashMap;

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
            mock_mode: true,
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
        self.pending_parameter_key = key;
        let mut args = vec!["ssm".into()];
        if path.starts_with('/') {
            // Hierarchical paths (including the root '/') use GetParametersByPath.
            args.extend(["get-parameters-by-path".into(), "--path".into(), path,
                "--recursive".into(), "--no-with-decryption".into()]);
        } else {
            // Parameter names without a leading slash are valid in SSM too.
            // DescribeParameters returns metadata only; values are still fetched on demand.
            args.extend(["describe-parameters".into(), "--parameter-filters".into(),
                format!("Key=Name,Option=BeginsWith,Values={path}")]);
        }
        args.extend(["--profile".into(), profile, "--region".into(), region, "--output".into(), "json".into()]);
        self.launch("Listar parâmetros SSM", "aws", args);
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

    fn field_combo(ui: &mut egui::Ui, label: &str, value: &mut String, filter: &mut String, values: &[String]) {
        ui.label(label);
        egui::ComboBox::from_id_salt(label)
            .close_behavior(egui::PopupCloseBehavior::IgnoreClicks)
            .selected_text(if value.is_empty() { "Selecionar…" } else { value.as_str() })
            .width(300.0)
            .height(400.0)
            .show_ui(ui, |ui| {
                ui.set_min_width(300.0);
                ui.set_min_height(28.0);
                ui.add(egui::TextEdit::singleline(filter).hint_text("Pesquisar / filtrar...").desired_width(280.0));
                ui.separator();
                let needle = filter.trim().to_lowercase();
                let mut found = false;
                for item in values.iter().filter(|item| item.to_lowercase().contains(&needle)) {
                    found = true;
                    if ui.add_sized([280.0, 34.0], egui::Button::new(item).selected(*value == *item)).clicked() {
                        *value = item.clone();
                        ui.close_menu();
                    }
                }
                if !found {
                    ui.label("Nenhum resultado para esse filtro.");
                }
            });
    }

    fn nav_button(&mut self, ui: &mut egui::Ui, section: Section) {
        if ui.selectable_label(self.section == section, section.label()).clicked() {
            self.section = section;
        }
    }
}

impl eframe::App for KubectlUtilApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Evita que áreas não cobertas pelos painéis fiquem pretas ao alternar o Mock.
        let screen_rect = ctx.screen_rect();
        let background = ctx.style().visuals.panel_fill;
        ctx.layer_painter(egui::LayerId::background()).rect_filled(
            screen_rect,
            0.0,
            background,
        );
        while let Ok(result) = self.rx.try_recv() {
            self.busy = false;
            self.status = if result.success { "Concluído".into() } else { "Com erro — veja a saída".into() };
            if result.title == "Revelar SecureString" || result.title == "Revelar parâmetro" {
                // Never put parameter values into the general output panel or operation history.
                self.revealed_parameter_value = if result.success { result.output.clone() } else { String::new() };
                self.parameter_value_visible = result.success;
                self.output_title = "Parameter Store — resultado seguro".into();
                self.output = if result.success { "Valor carregado na área segura do Parameter Store. O conteúdo não foi adicionado ao histórico nem ao log geral.".into() } else { result.output.clone() };
                self.history.push(format!("{} | consulta de valor de parâmetro | {}", timestamp(), if result.success { "OK" } else { "ERRO" }));
                if self.history.len() > 200 { self.history.remove(0); }
                continue;
            }
            self.output_title = result.title.clone();
            self.output = result.output.clone();
            self.history.push(format!("{} | {} | {}", timestamp(), result.title, if result.success { "OK" } else { "ERRO" }));
            if self.history.len() > 200 { self.history.remove(0); }
            if result.title == "Explorador de Pods" { self.explorer_output = result.output.clone(); }
            if result.title == "Diagnóstico de Pods" {
                self.output = diagnose_pods(&result.output);
                self.explorer_output = result.output.clone();
            }

            let lines: Vec<String> = result.output.lines().map(str::trim).filter(|s| !s.is_empty()).map(str::to_string).collect();
            match result.title.as_str() {
                "Contextos Kubernetes" => {
                    self.contexts = lines;
                    if self.context.is_empty() {
                        self.context = self.output.lines().next().unwrap_or("").trim().to_string();
                    }
                }
                "Namespaces" => {
                    self.namespaces = lines;
                    if !self.namespaces.contains(&self.namespace) { self.namespace.clear(); }
                }
                "Pods" => {
                    self.pods = lines;
                    self.pod.clear();
                }
                "Services" => {
                    self.services = lines;
                    self.service.clear();
                }
                "Perfis AWS" => {
                    self.profiles = lines;
                    if !self.profiles.contains(&self.profile) && !self.profiles.is_empty() {
                        self.profile = self.profiles[0].clone();
                    }
                }
                "Listar parâmetros SSM" => {
                    if !result.success && (result.output.contains("UnrecognizedClientException") || result.output.to_lowercase().contains("security token included in the request is invalid")) {
                        self.status = format!("Credenciais AWS inválidas ou expiradas para o perfil '{}'. Atualize a sessão e tente novamente. Se usa AWS SSO, execute `aws sso login --profile {}` no terminal; se usa credenciais temporárias, renove também AWS_SESSION_TOKEN. Confira o perfil e a região.", self.profile, self.profile);
                    } else if result.success {
                        let parsed = serde_json::from_str::<serde_json::Value>(&result.output);
                        match parsed {
                            Ok(json) => {
                                let entries: Vec<ParameterEntry> = json["Parameters"].as_array().into_iter().flatten().map(|p| ParameterEntry {
                                    name: p["Name"].as_str().unwrap_or("").to_string(),
                                    kind: p["Type"].as_str().unwrap_or("Unknown").to_string(),
                                    modified: p["LastModifiedDate"].as_str().unwrap_or("—").to_string(),
                                }).filter(|p| !p.name.is_empty()).collect();
                                let key = self.pending_parameter_key.clone();
                                self.parameter_cache.insert(key.clone(), entries.clone());
                                self.loaded_parameter_key = key;
                                self.parameters = entries;
                                self.selected_parameter.clear();
                                self.revealed_parameter_value.clear();
                                self.parameter_value_visible = false;
                                self.status = format!("{} parâmetros carregados. Valores SecureString não foram descriptografados.", self.parameters.len());
                            }
                            Err(e) => self.status = format!("Não foi possível interpretar o JSON do Parameter Store: {e}"),
                        }
                    }
                }
                _ => {}
            }
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(150));

        egui::SidePanel::left("navigation").exact_width(190.0).resizable(false).show(ctx, |ui| {
            ui.add_space(12.0);
            ui.heading("◈ kubectlUtil");
            ui.label(egui::RichText::new("Kubernetes & AWS").small().color(egui::Color32::GRAY));
            ui.add_space(10.0);
            let was_mock = self.mock_mode;
            ui.add_enabled_ui(!self.busy, |ui| {
                ui.checkbox(&mut self.mock_mode, "Modo Mock (sem rede)");
            });
            if self.mock_mode {
                ui.label(egui::RichText::new("Nenhum comando real será executado.").small().color(egui::Color32::LIGHT_GREEN));
                egui::ComboBox::from_label("Cenário de teste")
                    .selected_text(&self.mock_scenario)
                    .width(155.0)
                    .show_ui(ui, |ui| {
                        for scenario in ["Sucesso (dados padrão)", "Erro", "Timeout", "Lista vazia", "Sem permissões"] {
                            ui.selectable_value(&mut self.mock_scenario, scenario.to_string(), scenario);
                        }
                    });
            }
            if was_mock != self.mock_mode {
                self.status = if self.mock_mode { "Modo Mock ativado: operações externas bloqueadas." } else { "Modo real ativado: ações podem executar comandos AWS CLI/kubectl." }.into();
                if self.mock_mode {
                    for pf in &mut self.port_forwards {
                        if let Some(child) = pf.child.as_mut() {
                            let _ = child.kill();
                            let _ = child.wait();
                        }
                    }
                    self.port_forwards.clear();
                    self.refresh_contexts();
                } else {
                    self.port_forwards.retain(|pf| pf.child.is_some());
                }
            }
            ui.add_space(14.0);
            self.nav_button(ui, Section::Connection);
            self.nav_button(ui, Section::Overview);
            self.nav_button(ui, Section::Cluster);
            self.nav_button(ui, Section::Network);
            self.nav_button(ui, Section::Aws);
            self.nav_button(ui, Section::Parameters);
            ui.add_space(20.0);
            ui.separator();
            ui.label(egui::RichText::new("As consultas são feitas sob demanda.").small().color(egui::Color32::GRAY));
        });

        egui::TopBottomPanel::bottom("output_panel")
            .resizable(true)
            .default_height(350.0)
            .min_height(200.0)
            .show(ctx, |ui| self.show_output_panel(ui));

        // Garante o fundo do painel central conforme o tema do egui.
        let central_fill = ctx.style().visuals.panel_fill;
        egui::CentralPanel::default()
            .frame(egui::Frame::central_panel(&ctx.style()).fill(central_fill))
            .show(ctx, |ui| {
            ui.add_space(6.0);
            ui.heading(self.section.label());
            ui.label("Execute somente as consultas necessárias.");
            ui.add_space(12.0);

            
            if self.section != Section::Connection {
                ui.horizontal_wrapped(|ui| {
                    ui.label(format!(
                        "Contexto: {}",
                        if self.context.is_empty() {
                            "não selecionado"
                        } else {
                            &self.context
                        }
                    ));

                    ui.separator();

                    ui.label(format!(
                        "Namespace: {}",
                        if self.namespace.is_empty() {
                            "não selecionado"
                        } else {
                            &self.namespace
                        }
                    ));

                    if ui.button("Abrir conexão").clicked() {
                        self.section = Section::Connection;
                    }
                });

                ui.separator();
            }

            
            if self.section == Section::Connection {
                egui::Frame::group(ui.style()).show(ui, |ui| {
                    ui.heading("Conexão Kubernetes");

                    ui.horizontal_wrapped(|ui| {
                        Self::field_combo(ui, "Contexto", &mut self.context, &mut self.context_filter, &self.contexts);

                        if ui.button("Atualizar").clicked() {
                            self.refresh_contexts();
                        }

                        if ui.button("Usar contexto").clicked() {
                            let context = self.context.clone();

                            if !context.is_empty() {
                                self.launch(
                                    "Alterar contexto",
                                    "kubectl",
                                    vec![
                                        "config".into(),
                                        "use-context".into(),
                                        context,
                                    ],
                                );
                            } else {
                                self.status = "Selecione um contexto.".into();
                            }
                        }
                    });

                    ui.horizontal_wrapped(|ui| {
                        Self::field_combo(
                            ui,
                            "Namespace",
                            &mut self.namespace,
                            &mut self.namespace_filter,
                            &self.namespaces,
                        );

                        if ui.button("Carregar namespaces").clicked() {
                            self.refresh_namespaces();
                        }

                        if ui.button("Pods").clicked() {
                            self.refresh_workloads();
                        }

                        if ui.button("Services").clicked() {
                            self.load_services();
                        }
                    });

                    ui.horizontal_wrapped(|ui| {
                        ui.label("Pod");

                        egui::ComboBox::from_id_salt("pod")
                            .close_behavior(egui::PopupCloseBehavior::IgnoreClicks)
                            .selected_text(if self.pod.is_empty() { "Selecionar Pod…" } else { &self.pod })
                            .width(300.0).height(400.0)
                            .show_ui(ui, |ui| {
                                ui.set_min_width(300.0);
                                ui.set_min_height(28.0);
                                ui.add(egui::TextEdit::singleline(&mut self.pod_filter).hint_text("Pesquisar Pods...").desired_width(280.0));
                                ui.separator();
                                let needle = self.pod_filter.trim().to_lowercase();
                                let matches: Vec<String> = self.pods.iter().filter(|x| x.to_lowercase().contains(&needle)).cloned().collect();
                                if matches.is_empty() { ui.label("Nenhum Pod encontrado."); }
                                for item in matches { if ui.add_sized([280.0, 34.0], egui::Button::new(&item).selected(self.pod == item)).clicked() { self.pod = item; ui.close_menu(); } }
                            });

                        ui.label("Service");
                        egui::ComboBox::from_id_salt("service")
                            .close_behavior(egui::PopupCloseBehavior::IgnoreClicks)
                            .selected_text(if self.service.is_empty() { "Selecionar Service…" } else { &self.service })
                            .width(300.0).height(400.0)
                            .show_ui(ui, |ui| {
                                ui.set_min_width(300.0);
                                ui.set_min_height(28.0);
                                ui.add(egui::TextEdit::singleline(&mut self.service_filter).hint_text("Pesquisar Services...").desired_width(280.0));
                                ui.separator();
                                let needle = self.service_filter.trim().to_lowercase();
                                let matches: Vec<String> = self.services.iter().filter(|x| x.to_lowercase().contains(&needle)).cloned().collect();
                                if matches.is_empty() { ui.label("Nenhum Service encontrado."); }
                                for item in matches { if ui.add_sized([280.0, 34.0], egui::Button::new(&item).selected(self.service == item)).clicked() { self.service = item; ui.close_menu(); } }
                            });
                    });
                });

                ui.add_space(10.0);
            }


            ui.add_space(10.0);
            match self.section {
                Section::Overview => {
                    ui.label("Acesso rápido");
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Ver cluster-info").clicked() { self.k("Informações do cluster", &["cluster-info"]); }
                        if ui.button("Ver nodes").clicked() { self.k("Nodes", &["get", "nodes", "-o", "wide"]); }
                        if ui.button("Eventos").clicked() {
                            let ns = self.current_namespace();
                            if !ns.is_empty() { self.output_action("Eventos", vec!["get".into(),"events".into(),"-n".into(),ns,"--sort-by=.metadata.creationTimestamp".into()]); }
                            else { self.status = "Selecione um namespace.".into(); }
                        }
                    });
                }
                Section::Connection => {
                    ui.heading("Explorador e diagnóstico");
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Carregar/atualizar Pods (1 consulta)").clicked() { self.load_pod_explorer(); }
                        if ui.button("Diagnóstico rápido (1 consulta)").clicked() { self.run_pod_diagnostics(); }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Filtrar lista carregada:");
                        ui.add(egui::TextEdit::singleline(&mut self.filter).hint_text("nome, status, node…").desired_width(260.0));
                    });
                    if !self.explorer_output.is_empty() {
                        egui::ScrollArea::vertical()
                                .id_salt("workloads_pods_scroll_area")
                                .max_height(150.0)
                                .show(ui, |ui| {
                            for line in self.explorer_output.lines().filter(|l| l.to_lowercase().contains(&self.filter.to_lowercase())) {
                                ui.monospace(line);
                            }
                        });
                    }
                    ui.separator();
                    ui.heading("Troubleshooting");
                    ui.horizontal_wrapped(|ui| {
                        let ns = self.current_namespace();
                        let pod = self.current_pod();
                        if ui.button("Logs (últimas 100 linhas)").clicked() {
                            if !ns.is_empty() && !pod.is_empty() { self.output_action("Logs do Pod", vec!["logs".into(),"-n".into(),ns,pod,"--tail=100".into()]); }
                            else { self.status = "Selecione namespace e Pod.".into(); }
                        }
                        let ns = self.current_namespace(); let pod = self.current_pod();
                        if ui.button("Logs anteriores (--previous)").clicked() {
                            if !ns.is_empty() && !pod.is_empty() { self.output_action("Logs anteriores do Pod", vec!["logs".into(), "-n".into(), ns, pod, "--previous".into(), "--tail=100".into()]); }
                            else { self.status = "Selecione namespace e Pod.".into(); }
                        }
                        let ns = self.current_namespace(); let pod = self.current_pod();
                        if ui.button("Descrever Pod").clicked() {
                            if !ns.is_empty() && !pod.is_empty() { self.output_action("Describe Pod", vec!["describe".into(),"pod".into(),"-n".into(),ns,pod]); }
                            else { self.status = "Selecione namespace e Pod.".into(); }
                        }
                        let ns = self.current_namespace(); let pod = self.current_pod();
                        if ui.button("YAML do Pod").clicked() {
                            if !ns.is_empty() && !pod.is_empty() { self.output_action("YAML do Pod", vec!["get".into(),"pod".into(),"-n".into(),ns,pod,"-o".into(),"yaml".into()]); }
                            else { self.status = "Selecione namespace e Pod.".into(); }
                        }
                        let ns = self.current_namespace();
                        if ui.button("Top Pods").clicked() {
                            if !ns.is_empty() { self.output_action("Top Pods", vec!["top".into(),"pods".into(),"-n".into(),ns]); }
                            else { self.status = "Selecione namespace.".into(); }
                        }
                        let ns = self.current_namespace(); let pod = self.current_pod();
                        if ui.button("Bash no Pod").clicked() {
                            if !ns.is_empty() && !pod.is_empty() {
                                self.status = "Bash interativo requer terminal externo; use o comando abaixo.".into();
                                self.output = format!("kubectl exec -it -n {ns} {pod} -- bash\n\nExecute em um terminal separado.");
                            } else { self.status = "Selecione namespace e Pod.".into(); }
                        }
                    });
                    ui.separator();
                    ui.heading("Port Forward");
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Porta local"); ui.add(egui::TextEdit::singleline(&mut self.local_port).desired_width(75.0));
                        ui.label("Porta remota"); ui.add(egui::TextEdit::singleline(&mut self.remote_port).desired_width(75.0));
                    });
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Iniciar Port Forward do Pod").clicked() {
                            let (ns,pod,lp,rp) = (self.current_namespace(),self.current_pod(),self.local_port.clone(),self.remote_port.clone());
                            if !ns.is_empty() && !pod.is_empty() && valid_port(&lp) && valid_port(&rp) {
                                self.start_port_forward(pod);
                            } else { self.status = "Selecione namespace/Pod e informe portas válidas (1–65535).".into(); }
                        }
                        if ui.button("Iniciar Port Forward do Service").clicked() {
                            let (ns,svc,lp,rp) = (self.current_namespace(),self.current_service(),self.local_port.clone(),self.remote_port.clone());
                            if !ns.is_empty() && !svc.is_empty() && valid_port(&lp) && valid_port(&rp) {
                                self.start_port_forward(format!("svc/{svc}"));
                            } else { self.status = "Selecione namespace/Service e informe portas válidas.".into(); }
                        }
                    });
                    egui::Frame::group(ui.style())
                        .fill(egui::Color32::from_rgb(18, 75, 45))
                        .stroke(egui::Stroke::new(
                            2.0_f32,
                            egui::Color32::from_rgb(50, 220, 100),
                        ))
                        .inner_margin(egui::Margin::same(10))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new("●")
                                        .size(20.0)
                                        .strong()
                                        .color(egui::Color32::from_rgb(50, 255, 120)),
                                );

                                ui.vertical(|ui| {
                                    ui.heading(
                                        egui::RichText::new(format!(
                                            "{} PORT FORWARD(S) ATIVO(S)",
                                            self.port_forwards.len()
                                        ))
                                        .strong()
                                        .color(egui::Color32::LIGHT_GREEN),
                                    );

                                    ui.label("Conexões em execução — localhost");
                                });
                            });

                            ui.separator();

                            let mut stop_index: Option<usize> = None;

                            for (index, pf) in self.port_forwards.iter().enumerate() {
                                egui::Frame::group(ui.style())
                                    .fill(egui::Color32::from_rgb(25, 55, 40))
                                    .inner_margin(egui::Margin::same(6))
                                    .show(ui, |ui| {
                                        ui.horizontal_wrapped(|ui| {
                                            ui.label(
                                                egui::RichText::new(format!(
                                                    "{}:{} → {} ({})",
                                                    "localhost",
                                                    pf.local_port,
                                                    pf.remote_port,
                                                    pf.target
                                                ))
                                                .strong()
                                                .color(egui::Color32::WHITE),
                                            );

                                            ui.label(format!("Namespace: {}", pf.namespace));

                                            if ui.button(format!("■ Parar #{}", index + 1)).clicked() {
                                                stop_index = Some(index);
                                            }
                                        });
                                    });
                            }

                            if let Some(index) = stop_index {
                                self.stop_port_forward(index);
                            }
                        });
                }
                Section::Cluster => {
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Nodes (wide)").clicked() { self.k("Nodes", &["get","nodes","-o","wide"]); }
                        if ui.button("Top Nodes").clicked() { self.k("Top Nodes", &["top","nodes"]); }
                        if ui.button("Workloads").clicked() {
                            let ns=self.current_namespace();
                            if !ns.is_empty() { self.output_action("Workloads", vec!["get".into(),"deployments,daemonsets,statefulsets,jobs".into(),"-n".into(),ns]); }
                            else { self.status="Selecione namespace.".into(); }
                        }
                        if ui.button("Cluster info").clicked() { self.k("Informações do cluster", &["cluster-info"]); }
                        if ui.button("Eventos").clicked() {
                            let ns=self.current_namespace();
                            if !ns.is_empty() { self.output_action("Eventos", vec!["get".into(),"events".into(),"-n".into(),ns,"--sort-by=.metadata.creationTimestamp".into()]); }
                            else { self.status="Selecione namespace.".into(); }
                        }
                        if ui.button("Services").clicked() {
                            let ns=self.current_namespace();
                            if !ns.is_empty() { self.output_action("Services", vec!["get".into(),"svc".into(),"-n".into(),ns]); }
                            else { self.status="Selecione namespace.".into(); }
                        }
                        if ui.button("Imagens dos Pods").clicked() {
                            let ns=self.current_namespace();
                            if !ns.is_empty() { self.output_action("Imagens", vec!["get".into(),"pods".into(),"-n".into(),ns,"-o".into(),"jsonpath={range .items[*]}{.metadata.name}{\"\\t\"}{range .spec.containers[*]}{.image}{\" \"}{end}{\"\\n\"}{end}".into()]); }
                            else { self.status="Selecione namespace.".into(); }
                        }
                    });
                }
                Section::Network => {
                    ui.horizontal_wrapped(|ui| {
                        for (label, args) in [
                            ("Ingresses", vec!["get","ingress"]),
                            ("Detalhes de Ingress", vec!["describe","ingress"]),
                            ("Endpoints", vec!["get","endpoints"]),
                            ("PVC e PV", vec!["get","pvc,pv"]),
                            ("ConfigMaps", vec!["get","configmaps"]),
                            ("Backups Velero", vec!["get","backups"]),
                        ] {
                            if ui.button(label).clicked() {
                                let ns=self.current_namespace();
                                if !ns.is_empty() {
                                    let mut a:Vec<String> = args.iter().map(|s|s.to_string()).collect();
                                    a.extend(["-n".into(),ns]);
                                    self.output_action(label, a);
                                } else { self.status="Selecione namespace.".into(); }
                            }
                        }
                    });
                }
                Section::Aws => {
                    ui.heading("AWS & EKS");
                    ui.horizontal_wrapped(|ui| {
                        Self::field_combo(ui, "Perfil", &mut self.profile, &mut self.profile_filter, &self.profiles);
                        if ui.button("Carregar perfis").clicked() { self.load_profiles(); }
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Região");
                        ui.add(egui::TextEdit::singleline(&mut self.region).desired_width(130.0));
                        ui.label("Cluster EKS");
                        ui.add(egui::TextEdit::singleline(&mut self.cluster).desired_width(220.0));
                    });
                    ui.add_space(8.0);
                    ui.horizontal_wrapped(|ui| {
                        let p=self.profile.clone();
                        if ui.button("AWS configure list").clicked() {
                            self.aws("Configuração AWS", vec!["configure".into(),"list".into(),"--profile".into(),p]);
                        }
                        let p=self.profile.clone();
                        if ui.button("Identidade STS").clicked() {
                            self.aws("Identidade STS", vec!["sts".into(),"get-caller-identity".into(),"--profile".into(),p]);
                        }
                        let p=self.profile.clone(); let r=self.region.clone();
                        if ui.button("Listar clusters EKS").clicked() {
                            self.aws("Clusters EKS", vec!["eks".into(),"list-clusters".into(),"--profile".into(),p,"--region".into(),r]);
                        }
                        let p=self.profile.clone(); let r=self.region.clone(); let c=self.cluster.clone();
                        if ui.button("Atualizar kubeconfig").clicked() {
                            if c.trim().is_empty() { self.status="Informe o nome do cluster EKS.".into(); }
                            else { self.aws("Atualizar kubeconfig", vec!["eks".into(),"update-kubeconfig".into(),"--name".into(),c,"--profile".into(),p,"--region".into(),r]); }
                        }
                    });
                    ui.label("Login SSO: para manter a autenticação interativa, abra um terminal e execute:");
                    ui.horizontal(|ui| {
                        let sso = format!("aws sso login --profile {}", self.profile);
                        ui.add(egui::TextEdit::singleline(&mut sso.clone()).desired_width(390.0).interactive(false));
                        if ui.button("Copiar comando SSO").clicked() { ui.ctx().copy_text(sso); self.status="Comando SSO copiado.".into(); }
                    });
                    ui.label(egui::RichText::new("O login SSO não é iniciado dentro da interface nesta versão.").small().color(egui::Color32::GRAY));
                }
                Section::Parameters => {
                    ui.heading("AWS Systems Manager — Parameter Store");
                    ui.label("Somente leitura. O aplicativo não cria, altera nem exclui parâmetros.");
                    ui.label(format!("Perfil: {}    Região: {}", self.profile, self.region));
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Caminho");
                        ui.add(egui::TextEdit::singleline(&mut self.parameter_path).desired_width(300.0).hint_text("/producao/minha-aplicacao/"));
                        if ui.add_enabled(!self.busy, egui::Button::new("Carregar (usar cache)" )).clicked() { self.load_parameters(false); }
                        if ui.add_enabled(!self.busy, egui::Button::new("Atualizar da AWS")).clicked() { self.load_parameters(true); }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Filtrar nomes");
                        ui.add(egui::TextEdit::singleline(&mut self.parameter_filter).desired_width(280.0).hint_text("Digite parte do nome"));
                        ui.label(format!("{} parâmetros em memória", self.parameters.len()));
                    });
                        egui::ScrollArea::vertical()
                            .id_salt("parameter_store_scroll_area")
                            .max_height(220.0)
                            .show(ui, |ui| {
                        egui::Grid::new("ssm_parameters_grid").striped(true).show(ui, |ui| {
                            ui.strong("Nome"); ui.strong("Tipo"); ui.strong("Última modificação"); ui.end_row();
                            let filter = self.parameter_filter.to_lowercase();
                            let visible_parameters: Vec<ParameterEntry> = self.parameters.iter()
                                .filter(|p| p.name.to_lowercase().contains(&filter)).cloned().collect();
                            for p in visible_parameters {
                                if ui.selectable_label(self.selected_parameter == p.name, &p.name).clicked() {
                                    self.selected_parameter = p.name.clone();
                                    self.revealed_parameter_value.clear();
                                    self.parameter_value_visible = false;
                                }
                                ui.label(&p.kind);
                                ui.label(&p.modified);
                                ui.end_row();
                            }
                        });
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.label(if self.selected_parameter.is_empty() { "Nenhum parâmetro selecionado".to_string() } else { self.selected_parameter.clone() });
                        if ui.add_enabled(!self.busy && !self.selected_parameter.is_empty(), egui::Button::new("Consultar valor")).clicked() { self.reveal_selected_parameter(); }
                        if self.parameter_value_visible {
                            if ui.button("Ocultar valor").clicked() { self.parameter_value_visible = false; }
                            if ui.button("Copiar valor").clicked() { ui.ctx().copy_text(self.revealed_parameter_value.clone()); self.status = "Valor copiado para a área de transferência; limpe-a se necessário.".into(); }
                        }
                    });
                    if self.parameter_value_visible {
                        ui.label("Valor consultado sob demanda. Não é salvo no histórico nem no arquivo de log geral.");
                        ui.add(egui::TextEdit::multiline(&mut self.revealed_parameter_value).font(egui::TextStyle::Monospace).desired_width(f32::INFINITY).desired_rows(3).interactive(false));
                    } else if !self.selected_parameter.is_empty() {
                        ui.label("Valor oculto. Clique em ‘Consultar valor’ para fazer uma consulta AWS individual.");
                    }
                    ui.label(egui::RichText::new("Os resultados ficam em cache em memória por perfil, região e caminho até atualizar, trocar a consulta ou fechar o aplicativo. SecureString só é descriptografado quando você solicita o valor.").small().color(egui::Color32::GRAY));
                }
            }
            ui.add_space(8.0);
            egui::CollapsingHeader::new(format!("Histórico local de operações ({} entradas)", self.history.len())).default_open(false).show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Copiar histórico").clicked() { ui.ctx().copy_text(self.history.join("\n")); self.status = "Histórico copiado.".into(); }
                    if ui.button("Limpar histórico").clicked() { self.history.clear(); }
                });
                egui::ScrollArea::vertical()
                    .id_salt("history_scroll_area")
                    .max_height(100.0)
                    .show(ui, |ui| {
                    for item in self.history.iter().rev().take(50) { ui.monospace(item); }
                });
            });
        });
    }
}

fn mock_scenario_message(scenario: &str) -> String {
    match scenario {
        "Erro" => "[MOCK] Erro simulado: falha inesperada ao consultar o serviço.".into(),
        "Timeout" => "[MOCK] Timeout simulado: a operação excedeu o tempo limite.".into(),
        "Lista vazia" => "[MOCK] Consulta concluída; nenhum resultado encontrado.".into(),
        "Sem permissões" => "[MOCK] AccessDenied: usuário/perfil sem permissão para executar esta operação.".into(),
        _ => "[MOCK] Operação simulada.".into(),
    }
}

fn mock_command_result(title: String, program: &str, args: &[String], scenario: &str) -> CommandResult {
    if scenario == "Timeout" {
        thread::sleep(std::time::Duration::from_secs(3));
        return CommandResult { title, output: mock_scenario_message(scenario), success: false };
    }
    if scenario == "Erro" || scenario == "Sem permissões" {
        return CommandResult { title, output: mock_scenario_message(scenario), success: false };
    }
    if scenario == "Lista vazia" {
        let output = if title == "Listar parâmetros SSM" { r#"{"Parameters":[]}"#.to_string() }
            else if title == "Identidade STS" { "{}".into() }
            else if title == "Clusters EKS" { r#"{"clusters":[]}"#.into() }
            else { String::new() };
        return CommandResult { title, output, success: true };
    }

    let joined = args.iter().map(String::as_str).collect::<Vec<_>>().join(" ");
    let output = match title.as_str() {
        "Contextos Kubernetes" => "dev-cluster\nstaging-cluster\nprod-cluster".into(),
        "Namespaces" => "default\ndev\nstaging\nmonitoring".into(),
        "Pods" => "api-7d9f6c8b5d-a1b2c\nworker-6b8d7f9c4d-x2y3z\nweb-5c6d7e8f9a-m4n5o".into(),
        "Services" => "api-service\nweb-service\npostgres-service".into(),
        "Perfis AWS" => "default\ndev\nstaging".into(),
        "Listar parâmetros SSM" => r#"{"Parameters":[{"Name":"/dev/app/API_URL","Type":"String","LastModifiedDate":"2026-10-01T12:00:00Z"},{"Name":"/dev/app/DB_PASSWORD","Type":"SecureString","LastModifiedDate":"2026-10-02T09:30:00Z"},{"Name":"/dev/app/FEATURE_FLAGS","Type":"StringList","LastModifiedDate":"2026-10-03T14:15:00Z"}]}"#.into(),
        "Revelar SecureString" => "mock-secret-value-not-real".into(),
        "Revelar parâmetro" => "https://api.mock.example.local".into(),
        "Configuração AWS" => "      Name                    Value             Type    Location\n      ----                    -----             ----    --------\n   profile                    dev               manual  --profile\naccess_key     ****************MOCK              shared-credentials-file\nsecret_key     ****************MOCK              shared-credentials-file\n    region                sa-east-1              config-file".into(),
        "Identidade STS" => r#"{"UserId":"AIDAMOCKUSER","Account":"000000000000","Arn":"arn:aws:iam::000000000000:user/mock-user"}"#.into(),
        "Clusters EKS" => r#"{"clusters":["dev-eks","staging-eks"]}"#.into(),
        "Atualizar kubeconfig" | "Alterar contexto" => format!("[MOCK] Alteração simulada. Nenhuma configuração real foi modificada.\nAlvo: {joined}"),
        "Informações do cluster" => "Kubernetes control plane is running at https://mock-cluster.example.local\nCoreDNS is running at https://mock-cluster.example.local/api/v1/namespaces/kube-system/services/kube-dns:dns/proxy".into(),
        "Nodes" | "Top Nodes" => "NAME             STATUS   ROLES           AGE   VERSION\nmock-node-01     Ready    control-plane   12d   v1.31.0\nmock-node-02     Ready    <none>          12d   v1.31.0".into(),
        "Explorador de Pods" | "Diagnóstico de Pods" => "NAME                          READY   STATUS             RESTARTS   AGE   IP            NODE\napi-7d9f6c8b5d-a1b2c           1/1     Running            0          2d    10.0.1.12     mock-node-01\nworker-6b8d7f9c4d-x2y3z       0/1     CrashLoopBackOff   5          3h    10.0.2.18     mock-node-02\nweb-5c6d7e8f9a-m4n5o          1/1     Running            0          1d    10.0.1.25     mock-node-01".into(),
        "Logs do Pod" => "[MOCK] 2026-10-12T12:00:00Z INFO aplicação iniciada\n[MOCK] 2026-10-12T12:00:01Z WARN tentativa de conexão simulada\n[MOCK] Nenhum log real foi consultado.".into(),
        "Eventos" => "LAST SEEN   TYPE      REASON      OBJECT              MESSAGE\n2m          Normal    Scheduled   pod/api-mock         Successfully assigned dev/api-mock\n1m          Warning   BackOff     pod/worker-mock      Back-off restarting failed container".into(),
        "Workloads" => "NAME                READY   UP-TO-DATE   AVAILABLE   AGE\ndeployment.apps/api  2/2     2            2           5d\nstatefulset.apps/db  1/1     1            1           5d".into(),
        "Imagens" => "api-7d9f6c8b5d-a1b2c\tnginx:1.27\nworker-6b8d7f9c4d-x2y3z\tregistry.example.local/worker:2.1".into(),
        "Ingresses" => "NAME       CLASS   HOSTS                 ADDRESS       PORTS\napp        nginx   app.mock.local        127.0.0.1     80".into(),
        "Detalhes de Ingress" => "Name:             app\nNamespace:        dev\nHost:             app.mock.local\nAddress:          127.0.0.1".into(),
        "Endpoints" => "NAME           ENDPOINTS             AGE\napi-service    10.0.1.12:8080        5d\nweb-service    10.0.1.25:80           5d".into(),
        "PVC e PV" => "NAME        STATUS   VOLUME        CAPACITY   ACCESS MODES\ndata-db     Bound    pv-mock-01    10Gi       RWO".into(),
        "ConfigMaps" => "NAME             DATA   AGE\napp-config       4      5d\nfeature-flags    2      2d".into(),
        "Backups Velero" => "NAME                  STATUS      CREATED\nmock-backup-daily     Completed   2026-10-12T02:00:00Z".into(),
        _ if program == "aws" && joined.contains("get-parameter") => "mock-value-not-real".into(),
        _ => format!("[MOCK] {title}: resultado simulado para `{program} {joined}`. Nenhum comando real foi executado."),
    };
    CommandResult { title, output, success: true }
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
