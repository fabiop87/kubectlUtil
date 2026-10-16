//! Estado do aplicativo e operações. A interface fica em `ui/`, o tratamento de
//! resultados em `results.rs` e as execuções (reais ou simuladas) em `runner.rs`.

use crate::action::Action;
use crate::command::{self, CommandResult};
use eframe::egui;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;

mod diagnostics;
mod mock;
mod parameters;
mod port_forward;
mod presets;
mod results;
mod runner;
mod ui;
mod util;

use self::diagnostics::diagnose_pods;
use self::mock::MockScenario;
use self::parameters::ParameterStore;
use self::port_forward::{ActivePortForward, PortForwardManager, StopOutcome};
use self::presets::Preset;
use self::runner::{MockRunner, RealRunner, Runner};
use self::util::{timestamp, valid_port};

const HISTORY_LIMIT: usize = 200;
/// Limite de espera por resposta da API do cluster (evita `kubectl` preso).
const KUBECTL_REQUEST_TIMEOUT: &str = "30s";

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

/// Valor escolhido + opções disponíveis + texto do filtro de um combo pesquisável.
#[derive(Default)]
struct Selectable {
    value: String,
    items: Vec<String>,
    filter: String,
}

impl Selectable {
    fn with_value(value: &str) -> Self {
        Self { value: value.into(), ..Self::default() }
    }

    fn trimmed(&self) -> &str {
        self.value.trim()
    }
}

pub struct KubectlUtilApp {
    ctx: egui::Context,
    section: Section,
    context: Selectable,
    namespace: Selectable,
    pod: Selectable,
    service: Selectable,
    profile: Selectable,
    region: String,
    cluster: String,
    params: ParameterStore,
    local_port: String,
    remote_port: String,
    filter: String,
    output_title: String,
    output: String,
    status: String,
    busy: bool,
    /// Sinal de cancelamento da operação em andamento.
    cancel: Arc<AtomicBool>,
    mock_mode: bool,
    mock_scenario: MockScenario,
    /// Tema escuro ligado (o tema inicial é o claro).
    dark_mode: bool,
    explorer_output: String,
    explorer_namespace: String,
    history: Vec<String>,
    port_forwards: PortForwardManager,
    tx: Sender<CommandResult>,
    rx: Receiver<CommandResult>,
}

impl KubectlUtilApp {
    pub fn new(ctx: egui::Context) -> Self {
        let (tx, rx) = mpsc::channel();
        let mut app = Self {
            ctx,
            section: Section::Connection,
            context: Selectable::default(),
            namespace: Selectable::default(),
            pod: Selectable::default(),
            service: Selectable::default(),
            profile: Selectable::with_value("default"),
            region: "us-east-1".into(),
            cluster: String::new(),
            params: ParameterStore::new(),
            local_port: "5434".into(),
            remote_port: "5432".into(),
            filter: String::new(),
            output_title: "Saída dos comandos".into(),
            output: "Bem-vindo ao kubectlUtil.\n\nSelecione uma seção e execute uma ação para consultar a AWS CLI ou o kubectl.".into(),
            status: "Pronto".into(),
            busy: false,
            cancel: Arc::new(AtomicBool::new(false)),
            mock_mode: false,
            mock_scenario: MockScenario::Success,
            dark_mode: false,
            explorer_output: String::new(),
            explorer_namespace: String::new(),
            history: Vec::new(),
            port_forwards: PortForwardManager::default(),
            tx,
            rx,
        };
        app.refresh_contexts();
        app
    }

    // ------------------------------------------------------------------
    // Execução de comandos
    // ------------------------------------------------------------------

    /// Real ou simulado, conforme o Modo Mock.
    fn runner(&self) -> Arc<dyn Runner> {
        if self.mock_mode {
            Arc::new(MockRunner { scenario: self.mock_scenario })
        } else {
            Arc::new(RealRunner)
        }
    }

    /// Executa um comando em segundo plano; o resultado volta pelo canal `rx`.
    fn launch(&mut self, action: Action, program: &str, args: Vec<String>) {
        if self.busy {
            self.status = "Aguarde a operação atual terminar.".into();
            return;
        }
        self.busy = true;
        self.status = format!("Executando: {}…", action.title());
        if !action.is_silent() {
            self.output_title = action.title().into();
            self.output = "Consultando…".into();
        }

        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel = Arc::clone(&cancel);
        let runner = self.runner();
        let tx = self.tx.clone();
        let ctx = self.ctx.clone();
        let program = program.to_string();
        thread::spawn(move || {
            let result = runner.run(action, &program, &args, &cancel);
            let _ = tx.send(result);
            // Acorda a interface assim que o resultado estiver pronto.
            ctx.request_repaint();
        });
    }

    /// Argumentos globais do kubectl: contexto escolhido na interface e limite de espera.
    ///
    /// Passar `--context` em toda chamada garante que a consulta use exatamente o
    /// contexto exibido na tela, sem depender (nem alterar) o kubeconfig global.
    fn kubectl_args(&self, args: Vec<String>) -> Vec<String> {
        let mut full = Vec::with_capacity(args.len() + 2);
        let context = self.context.trimmed();
        if !context.is_empty() {
            full.push("--context".to_string());
            full.push(context.to_string());
        }
        full.push(format!("--request-timeout={KUBECTL_REQUEST_TIMEOUT}"));
        full.extend(args);
        full
    }

    fn kubectl(&mut self, action: Action, args: Vec<String>) {
        let args = self.kubectl_args(args);
        self.launch(action, "kubectl", args);
    }

    fn aws(&mut self, action: Action, args: Vec<String>) {
        self.launch(action, "aws", args);
    }

    /// `base` + `--profile` (+ `--region`).
    fn aws_args(&self, base: &[&str], with_region: bool) -> Vec<String> {
        let mut args = command::args(base);
        args.push("--profile".into());
        args.push(self.profile.trimmed().to_string());
        if with_region {
            args.push("--region".into());
            args.push(self.region.trim().to_string());
        }
        args
    }

    /// Namespace selecionado, ou aviso ao usuário se não houver.
    fn require_namespace(&mut self) -> Option<String> {
        let namespace = self.namespace.trimmed().to_string();
        if namespace.is_empty() {
            self.status = "Selecione um namespace.".into();
            None
        } else {
            Some(namespace)
        }
    }

    fn run_preset(&mut self, preset: &Preset) {
        let namespace = self.namespace.trimmed().to_string();
        let pod = self.pod.trimmed().to_string();
        if !preset.needs.is_satisfied(&namespace, &pod) {
            self.status = preset.needs.missing_message().into();
            return;
        }
        self.kubectl(preset.action, preset.build_args(&namespace, &pod));
    }

    fn log_history(&mut self, entry: String) {
        self.history.push(entry);
        if self.history.len() > HISTORY_LIMIT {
            let excess = self.history.len() - HISTORY_LIMIT;
            self.history.drain(..excess);
        }
    }

    // ------------------------------------------------------------------
    // Kubernetes
    // ------------------------------------------------------------------

    fn refresh_contexts(&mut self) {
        // Atualização explícita: nunca consulta o kubectl em segundo plano por conta própria.
        self.launch(
            Action::Contexts,
            "kubectl",
            command::args(&["config", "get-contexts", "-o", "name"]),
        );
    }

    /// Descobre o contexto ativo do kubeconfig (silencioso: não altera o painel de saída).
    fn fetch_current_context(&mut self) {
        self.launch(Action::CurrentContext, "kubectl", command::args(&["config", "current-context"]));
    }

    /// Grava o contexto escolhido como padrão do kubeconfig (afeta seus outros terminais).
    fn use_selected_context(&mut self) {
        let context = self.context.trimmed().to_string();
        if context.is_empty() {
            self.status = "Selecione um contexto.".into();
            return;
        }
        self.launch(
            Action::UseContext,
            "kubectl",
            vec!["config".into(), "use-context".into(), context],
        );
    }

    /// Namespaces, Pods e Services pertencem ao contexto anterior e deixam de valer.
    fn on_context_changed(&mut self) {
        self.namespace = Selectable::default();
        self.pod = Selectable::default();
        self.service = Selectable::default();
        self.explorer_output.clear();
        self.explorer_namespace.clear();
        self.status = "Contexto alterado: carregue os namespaces deste contexto.".into();
    }

    /// Dados do modo anterior (real ou simulado) não valem no novo modo.
    fn on_mock_toggled(&mut self) {
        if self.mock_mode {
            self.port_forwards.stop_all();
        } else {
            self.port_forwards.drop_simulated();
        }
        self.reset_connection_state();
        self.refresh_contexts();
        self.status = if self.mock_mode {
            "Modo Mock ativado: operações externas bloqueadas."
        } else {
            "Modo real ativado: ações podem executar comandos AWS CLI/kubectl."
        }
        .into();
    }

    fn reset_connection_state(&mut self) {
        self.context = Selectable::default();
        self.namespace = Selectable::default();
        self.pod = Selectable::default();
        self.service = Selectable::default();
        self.profile = Selectable::with_value("default");
        self.params.clear_all();
        self.explorer_output.clear();
        self.explorer_namespace.clear();
    }

    fn refresh_namespaces(&mut self) {
        self.kubectl(
            Action::Namespaces,
            command::args(&["get", "namespaces", "-o", "custom-columns=:metadata.name"]),
        );
    }

    fn load_pods(&mut self) {
        let Some(namespace) = self.require_namespace() else { return };
        self.kubectl(
            Action::PodNames,
            command::args(&["get", "pods", "-n", &namespace, "-o", "custom-columns=:metadata.name"]),
        );
    }

    fn load_services(&mut self) {
        let Some(namespace) = self.require_namespace() else { return };
        self.kubectl(
            Action::ServiceNames,
            command::args(&["get", "svc", "-n", &namespace, "-o", "custom-columns=:metadata.name"]),
        );
    }

    fn pods_wide_args(namespace: &str) -> Vec<String> {
        command::args(&["get", "pods", "-n", namespace, "-o", "wide"])
    }

    fn load_pod_explorer(&mut self) {
        let Some(namespace) = self.require_namespace() else { return };
        self.explorer_namespace = namespace.clone();
        self.kubectl(Action::PodExplorer, Self::pods_wide_args(&namespace));
    }

    fn run_pod_diagnostics(&mut self) {
        let Some(namespace) = self.require_namespace() else { return };
        // Reaproveita a última listagem; só consulta o kubectl se não houver uma.
        if !self.explorer_output.is_empty() && self.explorer_namespace == namespace {
            self.output_title = "Diagnóstico de Pods (cache local)".into();
            self.output = diagnose_pods(&self.explorer_output);
            self.status = "Diagnóstico calculado a partir da última lista carregada; nenhuma nova consulta.".into();
        } else {
            self.explorer_namespace = namespace.clone();
            self.kubectl(Action::PodDiagnostics, Self::pods_wide_args(&namespace));
        }
    }

    // ------------------------------------------------------------------
    // Port Forward
    // ------------------------------------------------------------------

    fn start_port_forward(&mut self, target: String) {
        let namespace = self.namespace.trimmed().to_string();
        let local_port = self.local_port.trim().to_string();
        let remote_port = self.remote_port.trim().to_string();
        if namespace.is_empty() || !valid_port(&local_port) || !valid_port(&remote_port) {
            self.status = "Selecione namespace e informe portas válidas (1–65535).".into();
            return;
        }
        if self.port_forwards.local_port_in_use(&local_port) {
            self.status = format!(
                "A porta local {local_port} já está sendo usada por outro Port Forward. Escolha outra porta local."
            );
            return;
        }

        let mut args: Vec<String> = Vec::new();
        let context = self.context.trimmed();
        if !context.is_empty() {
            args.push("--context".to_string());
            args.push(context.to_string());
        }
        args.push("port-forward".to_string());
        args.push("-n".to_string());
        args.push(namespace.clone());
        args.push(target.clone());
        args.push(format!("{local_port}:{remote_port}"));

        match self.runner().start_background("kubectl", &args) {
            Ok(backend) => {
                let simulated = backend.is_simulated();
                self.port_forwards.push(ActivePortForward {
                    target: target.clone(),
                    namespace: namespace.clone(),
                    local_port: local_port.clone(),
                    remote_port: remote_port.clone(),
                    backend,
                });
                if simulated {
                    self.status = format!(
                        "[MOCK] Port Forward simulado: {target} em {namespace}, localhost:{local_port} → porta remota {remote_port}. Nenhuma conexão foi aberta."
                    );
                    self.log_history(format!(
                        "{} | [MOCK] iniciar port-forward {target} {namespace} {local_port}:{remote_port}",
                        timestamp()
                    ));
                } else {
                    self.status = format!(
                        "Port Forward ativo: {target} em {namespace}, localhost:{local_port} → porta remota {remote_port}."
                    );
                    self.log_history(format!(
                        "{} | iniciar port-forward {target} {namespace} {local_port}:{remote_port}",
                        timestamp()
                    ));
                }
            }
            Err(message) => self.status = message,
        }
    }

    fn stop_port_forward(&mut self, index: usize) {
        match self.port_forwards.stop(index) {
            StopOutcome::NotFound => {
                self.status = "Port Forward não encontrado; atualize a lista.".into();
            }
            StopOutcome::Failed(target, error) => {
                self.status = format!("Não foi possível encerrar Port Forward {target}: {error}");
            }
            StopOutcome::Stopped(pf) => {
                self.status = if pf.backend.is_simulated() {
                    format!("[MOCK] Port Forward simulado encerrado: {} em localhost:{}.", pf.target, pf.local_port)
                } else {
                    format!("Port Forward encerrado: {} em localhost:{}.", pf.target, pf.local_port)
                };
                self.log_history(format!(
                    "{} | parar port-forward {} {}:{}",
                    timestamp(),
                    pf.target,
                    pf.local_port,
                    pf.remote_port
                ));
            }
        }
    }

    fn stop_all_port_forwards(&mut self) {
        let count = self.port_forwards.len();
        self.port_forwards.stop_all();
        self.status = format!("{count} Port Forward(s) encerrado(s).");
        self.log_history(format!("{} | parar todos os port-forwards ({count})", timestamp()));
    }

    /// Remove da lista (e explica) os port-forwards cujo processo terminou sozinho.
    fn poll_port_forwards(&mut self) {
        for ended in self.port_forwards.poll_exited() {
            let detail = if ended.stderr.is_empty() {
                String::new()
            } else {
                format!("\n\n[stderr]\n{}", ended.stderr)
            };
            self.status = format!(
                "Port Forward encerrado: {} em localhost:{}.",
                ended.target, ended.local_port
            );
            self.output_title = "Port Forward encerrado".into();
            self.output = format!(
                "O kubectl port-forward de {} (namespace {}, localhost:{} → {}) terminou sozinho ({}).{detail}",
                ended.target, ended.namespace, ended.local_port, ended.remote_port, ended.status
            );
            self.log_history(format!(
                "{} | port-forward encerrado {} {}:{} | {}",
                timestamp(),
                ended.target,
                ended.local_port,
                ended.remote_port,
                ended.status
            ));
        }
    }

    // ------------------------------------------------------------------
    // AWS
    // ------------------------------------------------------------------

    fn load_profiles(&mut self) {
        self.aws(Action::AwsProfiles, command::args(&["configure", "list-profiles"]));
    }

    /// `aws sso login`: abre o navegador e espera a autorização (com limite de tempo).
    fn perform_login(&mut self) {
        let args = self.aws_args(&["sso", "login"], false);
        self.aws(Action::AwsSsoLogin, args);
    }

    // ------------------------------------------------------------------
    // Parameter Store
    // ------------------------------------------------------------------

    fn load_parameters(&mut self, force_refresh: bool) {
        let profile = self.profile.trimmed().to_string();
        let region = self.region.trim().to_string();
        let path = self.params.path.trim().to_string();
        if profile.is_empty() || region.is_empty() || path.is_empty() {
            self.status = "Informe um perfil, uma região e um nome ou caminho de parâmetro.".into();
            return;
        }
        let key = parameters::cache_key(&profile, &region, &path);
        if !force_refresh {
            if let Some(cached) = self.params.cache.get(&key).cloned() {
                self.params.show(key, cached);
                self.status = format!(
                    "{} parâmetros carregados do cache local; nenhuma chamada à AWS.",
                    self.params.entries.len()
                );
                return;
            }
        }
        // Listagem somente de metadados: `describe-parameters` não retorna valores.
        self.params.pending_key = key;
        self.aws(Action::SsmList, parameters::describe_args(&profile, &region, &path));
    }

    fn clear_parameter_cache(&mut self) {
        self.params.clear_all();
        self.status = "Cache do Parameter Store e dados exibidos em memória foram limpos.".into();
    }

    fn reveal_selected_parameter(&mut self) {
        let current_key = parameters::cache_key(
            self.profile.trimmed(),
            &self.region,
            &self.params.path,
        );
        if self.params.loaded_key != current_key {
            self.status = "O perfil, a região ou o caminho mudou. Carregue os parâmetros para atualizar a seleção antes de consultar valores.".into();
            return;
        }
        if self.params.selected.is_empty() {
            self.status = "Selecione um parâmetro primeiro.".into();
            return;
        }
        let selected = self.params.selected.clone();
        let Some(entry) = self.params.entries.iter().find(|p| p.name == selected) else {
            self.status = "O parâmetro selecionado não está na lista atual.".into();
            return;
        };
        let secure = entry.kind == "SecureString";
        let name = entry.name.clone();

        let args = parameters::get_args(&name, secure, self.profile.trimmed(), self.region.trim());
        let action = if secure { Action::SsmRevealSecure } else { Action::SsmRevealPlain };
        self.params.hide_value();
        self.aws(action, args);
    }
}
