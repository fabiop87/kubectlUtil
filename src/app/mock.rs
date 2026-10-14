use crate::command::CommandResult;
use std::thread;

pub(super) fn mock_scenario_message(scenario: &str) -> String {
    match scenario {
        "Erro" => "[MOCK] Erro simulado: falha inesperada ao consultar o serviço.".into(),
        "Timeout" => "[MOCK] Timeout simulado: a operação excedeu o tempo limite.".into(),
        "Lista vazia" => "[MOCK] Consulta concluída; nenhum resultado encontrado.".into(),
        "Sem permissões" => "[MOCK] AccessDenied: usuário/perfil sem permissão para executar esta operação.".into(),
        _ => "[MOCK] Operação simulada.".into(),
    }
}

pub(super) fn mock_command_result(title: String, program: &str, args: &[String], scenario: &str) -> CommandResult {
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

