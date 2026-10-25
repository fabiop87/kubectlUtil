//! Resultados simulados para o Modo Mock.
//!
//! O `match` em `success_output` não tem curinga (`_`) de propósito: ao criar uma
//! nova `Action`, o compilador exige decidir qual é a saída simulada dela.

use crate::action::Action;
use crate::command::{CANCELLED_MESSAGE, CommandResult};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MockScenario {
    Success,
    Error,
    Timeout,
    EmptyList,
    NoPermission,
}

impl MockScenario {
    pub const ALL: [MockScenario; 5] = [
        Self::Success,
        Self::Error,
        Self::Timeout,
        Self::EmptyList,
        Self::NoPermission,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Success => "Sucesso (dados padrão)",
            Self::Error => "Erro",
            Self::Timeout => "Timeout",
            Self::EmptyList => "Lista vazia",
            Self::NoPermission => "Sem permissões",
        }
    }

    pub fn message(self) -> &'static str {
        match self {
            Self::Error => "[MOCK] Erro simulado: falha inesperada ao consultar o serviço.",
            Self::Timeout => "[MOCK] Timeout simulado: a operação excedeu o tempo limite.",
            Self::EmptyList => "[MOCK] Consulta concluída; nenhum resultado encontrado.",
            Self::NoPermission => {
                "[MOCK] AccessDenied: usuário/perfil sem permissão para executar esta operação."
            }
            Self::Success => "[MOCK] Operação simulada.",
        }
    }
}

pub fn mock_command_result(
    action: Action,
    program: &str,
    args: &[String],
    scenario: MockScenario,
    cancel: &AtomicBool,
) -> CommandResult {
    match scenario {
        MockScenario::Timeout => {
            if sleep_unless_cancelled(Duration::from_secs(3), cancel) {
                return CommandResult {
                    action,
                    output: CANCELLED_MESSAGE.into(),
                    success: false,
                };
            }
            CommandResult {
                action,
                output: scenario.message().into(),
                success: false,
            }
        }
        MockScenario::Error | MockScenario::NoPermission => CommandResult {
            action,
            output: scenario.message().into(),
            success: false,
        },
        MockScenario::EmptyList => CommandResult {
            action,
            output: empty_output(action),
            success: true,
        },
        MockScenario::Success => {
            let joined = args.join(" ");
            CommandResult {
                action,
                output: success_output(action, program, &joined),
                success: true,
            }
        }
    }
}

/// Dorme em pequenos passos; devolve `true` se foi cancelado antes do fim.
fn sleep_unless_cancelled(total: Duration, cancel: &AtomicBool) -> bool {
    let deadline = Instant::now() + total;
    while Instant::now() < deadline {
        if cancel.load(Ordering::Relaxed) {
            return true;
        }
        thread::sleep(Duration::from_millis(50));
    }
    cancel.load(Ordering::Relaxed)
}

fn empty_output(action: Action) -> String {
    match action {
        Action::ServiceOverview => r#"{"items":[]}"#.into(),
        Action::SsmList => r#"{"Parameters":[]}"#.into(),
        Action::AwsIdentity => "{}".into(),
        Action::EksClusters => r#"{"clusters":[]}"#.into(),
        _ => String::new(),
    }
}

fn generic_output(action: Action, program: &str, joined: &str) -> String {
    format!(
        "[MOCK] {}: resultado simulado para `{program} {joined}`. Nenhum comando real foi executado.",
        action.title()
    )
}

fn success_output(action: Action, program: &str, joined: &str) -> String {
    match action {
        Action::ServiceOverview => super::service::mock_snapshot(),
        Action::Contexts => "dev-cluster\nstaging-cluster\nprod-cluster".into(),
        Action::CurrentContext => "dev-cluster".into(),
        Action::UseContext | Action::UpdateKubeconfig => format!(
            "[MOCK] Alteração simulada. Nenhuma configuração real foi modificada.\nAlvo: {joined}"
        ),
        Action::Namespaces => "default\ndev\nstaging\nmonitoring".into(),
        Action::PodNames => "api-7d9f6c8b5d-a1b2c\nworker-6b8d7f9c4d-x2y3z\nweb-5c6d7e8f9a-m4n5o".into(),
        Action::ServiceNames => "api-service\nweb-service\npostgres-service".into(),
        Action::PodExplorer | Action::PodDiagnostics => "NAME                          READY   STATUS             RESTARTS   AGE   IP            NODE\napi-7d9f6c8b5d-a1b2c           1/1     Running            0          2d    10.0.1.12     mock-node-01\nworker-6b8d7f9c4d-x2y3z       0/1     CrashLoopBackOff   5          3h    10.0.2.18     mock-node-02\nweb-5c6d7e8f9a-m4n5o          1/1     Running            0          1d    10.0.1.25     mock-node-01".into(),
        Action::PodLogs | Action::PodLogsPrevious => "[MOCK] 2026-10-12T12:00:00Z INFO aplicação iniciada\n[MOCK] 2026-10-12T12:00:01Z WARN tentativa de conexão simulada\n[MOCK] Nenhum log real foi consultado.".into(),
        Action::DescribePod | Action::PodYaml | Action::TopPods | Action::AwsSsoLogin => {
            generic_output(action, program, joined)
        }
        Action::ClusterInfo => "Kubernetes control plane is running at https://mock-cluster.example.local\nCoreDNS is running at https://mock-cluster.example.local/api/v1/namespaces/kube-system/services/kube-dns:dns/proxy".into(),
        Action::Nodes | Action::TopNodes => "NAME             STATUS   ROLES           AGE   VERSION\nmock-node-01     Ready    control-plane   12d   v1.31.0\nmock-node-02     Ready    <none>          12d   v1.31.0".into(),
        Action::Events => "LAST SEEN   TYPE      REASON      OBJECT              MESSAGE\n2m          Normal    Scheduled   pod/api-mock         Successfully assigned dev/api-mock\n1m          Warning   BackOff     pod/worker-mock      Back-off restarting failed container".into(),
        Action::Workloads => "NAME                READY   UP-TO-DATE   AVAILABLE   AGE\ndeployment.apps/api  2/2     2            2           5d\nstatefulset.apps/db  1/1     1            1           5d".into(),
        Action::ServicesTable => "NAME               TYPE        CLUSTER-IP     EXTERNAL-IP   PORT(S)    AGE\napi-service        ClusterIP   10.96.0.10     <none>        8080/TCP   5d\nweb-service        ClusterIP   10.96.0.11     <none>        80/TCP     5d\npostgres-service   ClusterIP   10.96.0.12     <none>        5432/TCP   5d".into(),
        Action::Images => "api-7d9f6c8b5d-a1b2c\tnginx:1.27\nworker-6b8d7f9c4d-x2y3z\tregistry.example.local/worker:2.1".into(),
        Action::Ingresses => "NAME       CLASS   HOSTS                 ADDRESS       PORTS\napp        nginx   app.mock.local        127.0.0.1     80".into(),
        Action::IngressDetails => "Name:             app\nNamespace:        dev\nHost:             app.mock.local\nAddress:          127.0.0.1".into(),
        Action::Endpoints => "NAME           ENDPOINTS             AGE\napi-service    10.0.1.12:8080        5d\nweb-service    10.0.1.25:80           5d".into(),
        Action::PvcAndPv => "NAME        STATUS   VOLUME        CAPACITY   ACCESS MODES\ndata-db     Bound    pv-mock-01    10Gi       RWO".into(),
        Action::ConfigMaps => "NAME             DATA   AGE\napp-config       4      5d\nfeature-flags    2      2d".into(),
        Action::VeleroBackups => "NAME                  STATUS      CREATED\nmock-backup-daily     Completed   2026-10-12T02:00:00Z".into(),
        Action::AwsProfiles => "default\ndev\nstaging".into(),
        Action::AwsConfigure => "      Name                    Value             Type    Location\n      ----                    -----             ----    --------\n   profile                    dev               manual  --profile\naccess_key     ****************MOCK              shared-credentials-file\nsecret_key     ****************MOCK              shared-credentials-file\n    region                sa-east-1              config-file".into(),
        Action::AwsIdentity => r#"{"UserId":"AIDAMOCKUSER","Account":"000000000000","Arn":"arn:aws:iam::000000000000:user/mock-user"}"#.into(),
        Action::EksClusters => r#"{"clusters":["dev-eks","staging-eks"]}"#.into(),
        Action::SsmList => r#"{"Parameters":[{"Name":"/dev/app/API_URL","Type":"String","LastModifiedDate":"2026-10-01T12:00:00Z"},{"Name":"/dev/app/DB_PASSWORD","Type":"SecureString","LastModifiedDate":"2026-10-02T09:30:00Z"},{"Name":"/dev/app/FEATURE_FLAGS","Type":"StringList","LastModifiedDate":"2026-10-03T14:15:00Z"}]}"#.into(),
        Action::SsmRevealSecure => "mock-secret-value-not-real".into(),
        Action::SsmRevealPlain => "https://api.mock.example.local".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(action: Action, scenario: MockScenario) -> CommandResult {
        mock_command_result(
            action,
            "kubectl",
            &["get".to_string()],
            scenario,
            &AtomicBool::new(false),
        )
    }

    #[test]
    fn success_returns_default_data() {
        let result = run(Action::Namespaces, MockScenario::Success);
        assert!(result.success);
        assert!(result.output.contains("monitoring"));
    }

    #[test]
    fn error_and_permission_scenarios_fail_with_message() {
        for scenario in [MockScenario::Error, MockScenario::NoPermission] {
            let result = run(Action::Namespaces, scenario);
            assert!(!result.success);
            assert_eq!(result.output, scenario.message());
        }
    }

    #[test]
    fn empty_list_keeps_json_shape_for_ssm() {
        let result = run(Action::SsmList, MockScenario::EmptyList);
        assert!(result.success);
        assert_eq!(result.output, r#"{"Parameters":[]}"#);
        assert!(
            run(Action::Namespaces, MockScenario::EmptyList)
                .output
                .is_empty()
        );
    }

    #[test]
    fn timeout_scenario_can_be_cancelled() {
        let cancel = AtomicBool::new(true);
        let started = Instant::now();
        let result = mock_command_result(
            Action::Nodes,
            "kubectl",
            &[],
            MockScenario::Timeout,
            &cancel,
        );
        assert!(!result.success);
        assert_eq!(result.output, CANCELLED_MESSAGE);
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn update_actions_show_the_target() {
        let result = mock_command_result(
            Action::UseContext,
            "kubectl",
            &[
                "config".to_string(),
                "use-context".to_string(),
                "dev".to_string(),
            ],
            MockScenario::Success,
            &AtomicBool::new(false),
        );
        assert!(result.output.contains("Alvo: config use-context dev"));
    }

    #[test]
    fn scenario_labels_are_unique() {
        let mut labels: Vec<&str> = MockScenario::ALL.iter().map(|s| s.label()).collect();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), MockScenario::ALL.len());
    }
}
