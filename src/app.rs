use crate::command::{self, CommandResult};
use eframe::egui;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::process::Child;

#[derive(PartialEq, Clone, Copy)]
enum Section {
    Overview,
    Workloads,
    Cluster,
    Network,
    Aws,
}

impl Section {
    fn label(self) -> &'static str {
        match self {
            Self::Overview => "Visão geral",
            Self::Workloads => "Pods e Services",
            Self::Cluster => "Cluster",
            Self::Network => "Rede e storage",
            Self::Aws => "AWS & EKS",
        }
    }
}

pub struct KubectlUtilApp {
    section: Section,
    context: String,
    contexts: Vec<String>,
    namespace: String,
    namespaces: Vec<String>,
    pod: String,
    pods: Vec<String>,
    service: String,
    services: Vec<String>,
    profile: String,
    profiles: Vec<String>,
    region: String,
    cluster: String,
    local_port: String,
    remote_port: String,
    filter: String,
    output_title: String,
    output: String,
    status: String,
    busy: bool,
    explorer_output: String,
    explorer_namespace: String,
    history: Vec<String>,
    port_forward: Option<Child>,
    port_forward_target: String,
    tx: Sender<CommandResult>,
    rx: Receiver<CommandResult>,
}

impl Default for KubectlUtilApp {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        let mut app = Self {
            section: Section::Overview,
            context: String::new(),
            contexts: vec![],
            namespace: String::new(),
            namespaces: vec![],
            pod: String::new(),
            pods: vec![],
            service: String::new(),
            services: vec![],
            profile: "default".into(),
            profiles: vec![],
            region: "us-east-1".into(),
            cluster: String::new(),
            local_port: "5434".into(),
            remote_port: "5432".into(),
            filter: String::new(),
            output_title: "Saída dos comandos".into(),
            output: "Bem-vindo ao kubectlUtil.\n\nSelecione uma seção e execute uma ação para consultar a AWS CLI ou o kubectl.".into(),
            status: "Pronto".into(),
            busy: false,
            explorer_output: String::new(),
            explorer_namespace: String::new(),
            history: Vec::new(),
            port_forward: None,
            port_forward_target: String::new(),
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
        thread::spawn(move || {
            let result = command::run(title, &program, &args);
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
        if self.port_forward.is_some() {
            self.status = "Já existe um Port Forward iniciado pelo aplicativo. Pare-o antes de iniciar outro.".into();
            return;
        }
        let ns = self.current_namespace();
        let lp = self.local_port.clone();
        let rp = self.remote_port.clone();
        if ns.is_empty() || !valid_port(&lp) || !valid_port(&rp) {
            self.status = "Selecione namespace e informe portas válidas (1–65535).".into(); return;
        }
        let args = vec!["port-forward".into(), "-n".into(), ns, target.clone(), format!("{lp}:{rp}")];
        match command::spawn("kubectl", &args) {
            Ok(child) => {
                self.port_forward = Some(child);
                self.port_forward_target = target.clone();
                self.status = format!("Port Forward iniciado para {target}. O processo ficará ativo até parar ou fechar o app.");
                self.history.push(format!("{} | iniciar port-forward {}", timestamp(), target));
            }
            Err(e) => self.status = format!("Falha ao iniciar Port Forward: {e}"),
        }
    }

    fn stop_port_forward(&mut self) {
        if let Some(mut child) = self.port_forward.take() {
            match child.kill() {
                Ok(()) => { let _ = child.wait(); self.status = "Port Forward encerrado.".into(); self.history.push(format!("{} | parar port-forward {}", timestamp(), self.port_forward_target)); }
                Err(e) => self.status = format!("Não foi possível encerrar Port Forward: {e}"),
            }
            self.port_forward_target.clear();
        } else { self.status = "Nenhum Port Forward iniciado pelo aplicativo.".into(); }
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
        egui::ScrollArea::vertical().max_height(230.0).stick_to_bottom(true).show(ui, |ui| {
            ui.add(egui::TextEdit::multiline(&mut self.output)
                .font(egui::TextStyle::Monospace)
                .desired_width(f32::INFINITY)
                .desired_rows(10)
                .interactive(false));
        });
    }

    fn field_combo(ui: &mut egui::Ui, label: &str, value: &mut String, values: &[String]) {
        ui.label(label);
        egui::ComboBox::from_id_salt(label)
            .selected_text(if value.is_empty() { "Selecionar…" } else { value.as_str() })
            .width(240.0)
            .show_ui(ui, |ui| {
                for item in values {
                    ui.selectable_value(value, item.clone(), item);
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
        while let Ok(result) = self.rx.try_recv() {
            self.busy = false;
            self.status = if result.success { "Concluído".into() } else { "Com erro — veja a saída".into() };
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
                _ => {}
            }
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(150));

        egui::SidePanel::left("navigation").exact_width(190.0).resizable(false).show(ctx, |ui| {
            ui.add_space(12.0);
            ui.heading("◈ kubectlUtil");
            ui.label(egui::RichText::new("Kubernetes & AWS").small().color(egui::Color32::GRAY));
            ui.add_space(24.0);
            self.nav_button(ui, Section::Overview);
            self.nav_button(ui, Section::Workloads);
            self.nav_button(ui, Section::Cluster);
            self.nav_button(ui, Section::Network);
            self.nav_button(ui, Section::Aws);
            ui.add_space(20.0);
            ui.separator();
            ui.label(egui::RichText::new("As consultas são feitas sob demanda.").small().color(egui::Color32::GRAY));
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(6.0);
            ui.heading(self.section.label());
            ui.label("Execute somente as consultas necessárias.");
            ui.add_space(12.0);

            egui::Frame::group(ui.style()).show(ui, |ui| {
                ui.heading("Conexão Kubernetes");
                ui.horizontal_wrapped(|ui| {
                    Self::field_combo(ui, "Contexto", &mut self.context, &self.contexts);
                    if ui.button("Atualizar").clicked() { self.refresh_contexts(); }
                    if ui.button("Usar contexto").clicked() {
                        let context = self.context.clone();
                        if !context.is_empty() {
                            self.launch("Alterar contexto", "kubectl", vec!["config".into(), "use-context".into(), context]);
                        } else { self.status = "Selecione um contexto.".into(); }
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    Self::field_combo(ui, "Namespace", &mut self.namespace, &self.namespaces);
                    if ui.button("Carregar namespaces").clicked() { self.refresh_namespaces(); }
                    if ui.button("Pods").clicked() { self.refresh_workloads(); }
                    if ui.button("Services").clicked() { self.load_services(); }
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label("Pod");
                    egui::ComboBox::from_id_salt("pod")
                        .selected_text(if self.pod.is_empty() { "Selecionar…" } else { &self.pod })
                        .width(260.0)
                        .show_ui(ui, |ui| {
                            for item in &self.pods { ui.selectable_value(&mut self.pod, item.clone(), item); }
                        });
                    ui.label("Service");
                    egui::ComboBox::from_id_salt("service")
                        .selected_text(if self.service.is_empty() { "Selecionar…" } else { &self.service })
                        .width(220.0)
                        .show_ui(ui, |ui| {
                            for item in &self.services { ui.selectable_value(&mut self.service, item.clone(), item); }
                        });
                });
            });

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
                Section::Workloads => {
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
                        egui::ScrollArea::vertical().max_height(150.0).show(ui, |ui| {
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
                        if ui.button("Parar Port Forward").clicked() { self.stop_port_forward(); }
                        if self.port_forward.is_some() { ui.label(format!("Ativo: {}", self.port_forward_target)); }
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
                        Self::field_combo(ui, "Perfil", &mut self.profile, &self.profiles);
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
            }
            ui.add_space(8.0);
            egui::CollapsingHeader::new(format!("Histórico local de operações ({} entradas)", self.history.len())).default_open(false).show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Copiar histórico").clicked() { ui.ctx().copy_text(self.history.join("\n")); self.status = "Histórico copiado.".into(); }
                    if ui.button("Limpar histórico").clicked() { self.history.clear(); }
                });
                egui::ScrollArea::vertical().max_height(100.0).show(ui, |ui| {
                    for item in self.history.iter().rev().take(50) { ui.monospace(item); }
                });
            });
            ui.add_space(6.0);
            self.show_output_panel(ui);
        });
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
        if let Some(child) = self.port_forward.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
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
