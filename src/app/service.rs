//! Visão local de um Service, sem executar comandos dentro dos containers.
use super::{KubectlUtilApp, Section};
use crate::{action::Action, command};
use serde_json::Value;
use std::collections::HashSet;

impl KubectlUtilApp {
    pub(super) fn load_service_overview(&mut self) {
        if self.busy {
            return;
        }
        let Some(namespace) = self.require_namespace() else {
            return;
        };
        if self.context.trimmed().is_empty() || self.service.trimmed().is_empty() {
            self.status = "Selecione contexto, namespace e Service.".into();
            return;
        }
        self.service_target = self.service.trimmed().to_string();
        self.kubectl(
            Action::ServiceOverview,
            command::args(&[
                "get",
                "services,pods,deployments,replicasets,statefulsets,daemonsets,ingresses,events",
                "-n",
                &namespace,
                "-o",
                "json",
            ]),
        );
    }

    pub(super) fn open_service_parameters(&mut self) {
        if self.context.trimmed().is_empty()
            || self.namespace.trimmed().is_empty()
            || self.service.trimmed().is_empty()
        {
            self.status = "Selecione contexto, namespace e Service.".into();
            return;
        }
        let key = format!(
            "{}/{}/{}",
            self.context.trimmed(),
            self.namespace.trimmed(),
            self.service.trimmed()
        );
        let result = std::fs::read_to_string("service-paths.json")
            .map_err(|e| format!("Não foi possível ler service-paths.json: {e}"))
            .and_then(|json| parameter_path(&json, &key));
        match result {
            Ok(path) => {
                self.params.hide_value();
                self.params.path = path;
                self.navigate(Section::Parameters);
                self.status =
                    "Caminho configurado. Confira perfil e região antes de carregar os parâmetros."
                        .into();
            }
            Err(error) => self.status = error,
        }
    }
}

fn parameter_path(json: &str, key: &str) -> Result<String, String> {
    let config: Value =
        serde_json::from_str(json).map_err(|e| format!("service-paths.json inválido: {e}"))?;
    let path = config.get(key).and_then(Value::as_str).unwrap_or("").trim();
    if !path.starts_with('/') || path.len() < 2 {
        return Err(format!(
            "Configure um caminho absoluto para '{key}' em service-paths.json."
        ));
    }
    Ok(path.into())
}

fn text<'a>(value: &'a Value, pointer: &str) -> &'a str {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .unwrap_or("—")
}

fn list<'a>(value: &'a Value, pointer: &str) -> &'a [Value] {
    value
        .pointer(pointer)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn number(value: &Value, pointer: &str) -> u64 {
    value.pointer(pointer).and_then(Value::as_u64).unwrap_or(0)
}

fn selected(selector: &serde_json::Map<String, Value>, labels: &Value) -> bool {
    !selector.is_empty()
        && selector
            .iter()
            .all(|(key, value)| labels.get(key) == Some(value))
}

pub(super) fn report(json: &str, service_name: &str) -> Result<String, String> {
    let root: Value =
        serde_json::from_str(json).map_err(|e| format!("JSON Kubernetes inválido: {e}"))?;
    let items = root
        .get("items")
        .and_then(Value::as_array)
        .ok_or("Resposta Kubernetes sem lista items.")?;
    let service = items
        .iter()
        .find(|v| text(v, "/kind") == "Service" && text(v, "/metadata/name") == service_name)
        .ok_or_else(|| {
            format!("Service '{service_name}' não encontrado no namespace consultado.")
        })?;
    let mut lines = vec![
        format!(
            "Service: {service_name} · namespace: {}",
            text(service, "/metadata/namespace")
        ),
        format!(
            "Tipo: {} · ClusterIP: {}",
            text(service, "/spec/type"),
            text(service, "/spec/clusterIP")
        ),
    ];
    for port in list(service, "/spec/ports") {
        lines.push(format!(
            "Porta: {} → {} ({})",
            port["port"],
            port["targetPort"],
            text(port, "/protocol")
        ));
    }
    let selector = service.pointer("/spec/selector").and_then(Value::as_object);
    let pods: Vec<_> = items
        .iter()
        .filter(|v| {
            text(v, "/kind") == "Pod"
                && selector.is_some_and(|s| selected(s, &v["metadata"]["labels"]))
        })
        .collect();
    let mut related: HashSet<String> = HashSet::new();
    for resource in std::iter::once(service).chain(pods.iter().copied()) {
        if let Some(uid) = resource["metadata"]["uid"].as_str() {
            related.insert(uid.into());
        }
    }
    // Segue Pod → ReplicaSet → Deployment por UID, sem inferir nomes.
    loop {
        let before = related.len();
        for resource in items {
            if resource["metadata"]["uid"]
                .as_str()
                .is_some_and(|uid| related.contains(uid))
            {
                for owner in list(resource, "/metadata/ownerReferences") {
                    if let Some(uid) = owner["uid"].as_str() {
                        related.insert(uid.into());
                    }
                }
            }
        }
        if related.len() == before {
            break;
        }
    }
    lines.push("\nWorkloads relacionados:".into());
    let mut workloads = 0;
    for resource in items {
        let kind = text(resource, "/kind");
        if !matches!(
            kind,
            "Deployment" | "StatefulSet" | "DaemonSet" | "ReplicaSet"
        ) {
            continue;
        }
        let linked = resource["metadata"]["uid"]
            .as_str()
            .is_some_and(|uid| related.contains(uid));
        // Inclui workloads com zero Pods usando os labels do template.
        if !linked
            && !selector
                .is_some_and(|s| selected(s, &resource["spec"]["template"]["metadata"]["labels"]))
        {
            continue;
        }
        workloads += 1;
        if let Some(uid) = resource["metadata"]["uid"].as_str() {
            related.insert(uid.into());
        }
        let (ready, desired) = if kind == "DaemonSet" {
            (
                number(resource, "/status/numberReady"),
                number(resource, "/status/desiredNumberScheduled"),
            )
        } else {
            (
                number(resource, "/status/readyReplicas"),
                resource
                    .pointer("/spec/replicas")
                    .and_then(Value::as_u64)
                    .unwrap_or(1),
            )
        };
        lines.push(format!(
            "{} {}: {ready}/{desired} prontos{}",
            kind,
            text(resource, "/metadata/name"),
            if ready < desired { " [ATENÇÃO]" } else { "" }
        ));
        for condition in list(resource, "/status/conditions") {
            if (text(condition, "/type") == "Available" && text(condition, "/status") == "False")
                || (text(condition, "/type") == "Progressing"
                    && text(condition, "/status") == "False")
            {
                lines.push(format!("  Condição: {}", text(condition, "/reason")));
            }
        }
    }
    if workloads == 0 {
        lines.push("Nenhum workload relacionado encontrado.".into());
    }
    lines.push(format!("\nPods selecionados: {}", pods.len()));
    if selector.is_none_or(|s| s.is_empty()) {
        lines.push("[LIMITE] Service sem selector: Pods e workloads não podem ser associados automaticamente.".into());
    } else if pods.is_empty() {
        lines.push("[ATENÇÃO] Nenhum Pod corresponde ao selector do Service.".into());
    }
    let mut ready_pods = 0;
    for pod in &pods {
        let ready = list(pod, "/status/conditions")
            .iter()
            .any(|c| text(c, "/type") == "Ready" && text(c, "/status") == "True");
        let terminating = pod
            .pointer("/metadata/deletionTimestamp")
            .is_some_and(|v| !v.is_null());
        if ready && !terminating {
            ready_pods += 1;
        }
        lines.push(format!(
            "Pod {}: {} · Ready={}{}",
            text(pod, "/metadata/name"),
            text(pod, "/status/phase"),
            ready,
            if terminating { " · encerrando" } else { "" }
        ));
        for pointer in ["/status/initContainerStatuses", "/status/containerStatuses"] {
            for container in list(pod, pointer) {
                lines.push(format!(
                    "  Container {}: pronto={} · reinícios={}",
                    text(container, "/name"),
                    container["ready"],
                    number(container, "/restartCount")
                ));
                for state in [
                    "/state/waiting/reason",
                    "/state/terminated/reason",
                    "/lastState/terminated/reason",
                ] {
                    if let Some(reason) = container.pointer(state).and_then(Value::as_str) {
                        lines.push(format!("    Evidência {state}: {reason}"));
                    }
                }
            }
        }
        for container in list(pod, "/spec/containers") {
            lines.push(format!(
                "  Probes {}: readiness={} · liveness={} · startup={}",
                text(container, "/name"),
                container.get("readinessProbe").is_some(),
                container.get("livenessProbe").is_some(),
                container.get("startupProbe").is_some()
            ));
        }
    }
    if !pods.is_empty() && ready_pods == 0 {
        lines.push("[ATENÇÃO] Nenhum Pod selecionado está Ready e fora de encerramento.".into());
    }
    lines.push("\nIngresses relacionados:".into());
    let mut ingresses = 0;
    for ingress in items.iter().filter(|v| text(v, "/kind") == "Ingress") {
        let default_matches = text(ingress, "/spec/defaultBackend/service/name") == service_name;
        let rules_match = list(ingress, "/spec/rules").iter().any(|rule| {
            list(rule, "/http/paths")
                .iter()
                .any(|p| text(p, "/backend/service/name") == service_name)
        });
        if default_matches || rules_match {
            ingresses += 1;
            if let Some(uid) = ingress["metadata"]["uid"].as_str() {
                related.insert(uid.into());
            }
            lines.push(text(ingress, "/metadata/name").into());
        }
    }
    if ingresses == 0 {
        lines.push("Nenhum Ingress aponta para este Service.".into());
    }
    lines.push("\nEventos relacionados (últimos 20 disponíveis):".into());
    let mut events: Vec<_> = items
        .iter()
        .filter(|v| {
            text(v, "/kind") == "Event"
                && v.pointer("/involvedObject/uid")
                    .and_then(Value::as_str)
                    .is_some_and(|uid| related.contains(uid))
        })
        .collect();
    events.sort_by_key(|v| {
        v.pointer("/lastTimestamp")
            .and_then(Value::as_str)
            .or_else(|| v.pointer("/eventTime").and_then(Value::as_str))
            .unwrap_or(text(v, "/metadata/creationTimestamp"))
    });
    if events.is_empty() {
        lines.push("Nenhum evento relacionado disponível.".into());
    }
    for event in events.iter().rev().take(20) {
        lines.push(format!(
            "{} · {} {}: {} — {}",
            text(event, "/type"),
            text(event, "/involvedObject/kind"),
            text(event, "/involvedObject/name"),
            text(event, "/reason"),
            text(event, "/message")
        ));
    }
    lines.push("\nDiagnóstico pontual: probes configuradas não comprovam saúde; eventos podem ter expirado. Não verifica conectividade nem executa comandos nos containers.".into());
    Ok(lines.join("\n"))
}

pub(super) fn mock_snapshot() -> String {
    serde_json::json!({"items": [
        {"kind":"Service","metadata":{"name":"api-service","namespace":"dev","uid":"svc"},"spec":{"type":"ClusterIP","clusterIP":"10.96.0.10","selector":{"app":"api"},"ports":[{"port":80,"targetPort":8080,"protocol":"TCP"}]}},
        {"kind":"Deployment","metadata":{"name":"api","uid":"deploy"},"spec":{"replicas":2,"template":{"metadata":{"labels":{"app":"api"}}}},"status":{"readyReplicas":0}},
        {"kind":"ReplicaSet","metadata":{"name":"api-rs","uid":"rs","ownerReferences":[{"uid":"deploy"}]},"spec":{"replicas":2},"status":{"readyReplicas":0}},
        {"kind":"Pod","metadata":{"name":"api-pod","uid":"pod","labels":{"app":"api"},"ownerReferences":[{"uid":"rs"}]},"spec":{"containers":[{"name":"api","readinessProbe":{}}]},"status":{"phase":"Running","conditions":[{"type":"Ready","status":"False"}],"containerStatuses":[{"name":"api","ready":false,"restartCount":3,"state":{"waiting":{"reason":"CrashLoopBackOff"}},"lastState":{"terminated":{"reason":"OOMKilled"}}}]}},
        {"kind":"Ingress","metadata":{"name":"api-ingress","uid":"ingress"},"spec":{"rules":[{"http":{"paths":[{"backend":{"service":{"name":"api-service"}}}]}}]}},
        {"kind":"Event","metadata":{"creationTimestamp":"2026-10-25T12:00:00Z"},"involvedObject":{"uid":"pod","kind":"Pod","name":"api-pod"},"type":"Warning","reason":"Unhealthy","message":"Readiness probe failed"}
    ]}).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integrates_owners_health_ingress_and_events_without_dumping_secrets() {
        let mut snapshot: Value = serde_json::from_str(&mock_snapshot()).unwrap();
        snapshot["items"][3]["spec"]["containers"][0]["env"] =
            serde_json::json!([{"name":"PASSWORD","value":"do-not-print"}]);
        snapshot["items"].as_array_mut().unwrap().push(serde_json::json!({"kind":"Pod","metadata":{"name":"unrelated","labels":{"app":"other"}}}));
        let output = report(&snapshot.to_string(), "api-service").unwrap();
        for expected in [
            "Deployment api",
            "ReplicaSet api-rs",
            "CrashLoopBackOff",
            "OOMKilled",
            "api-ingress",
            "Readiness probe failed",
            "Nenhum Pod selecionado está Ready",
        ] {
            assert!(output.contains(expected), "{expected}");
        }
        assert!(!output.contains("unrelated"));
        assert!(!output.contains("do-not-print"));
    }

    #[test]
    fn handles_empty_missing_and_selectorless_services() {
        assert!(report("invalid", "api").is_err());
        assert!(report(r#"{"items":[]}"#, "api").is_err());
        let mut snapshot: Value = serde_json::from_str(&mock_snapshot()).unwrap();
        snapshot["items"][0]["spec"]["selector"] = serde_json::json!({});
        let output = report(&snapshot.to_string(), "api-service").unwrap();
        assert!(output.contains("Service sem selector"));
        assert!(output.contains("Pods selecionados: 0"));
        assert!(!output.contains("Deployment api"));
    }

    #[test]
    fn parameter_mapping_is_scoped_and_validated() {
        let config = r#"{"prod/team/api":"/prod/api","dev/team/api":"/dev/api","bad":"relative"}"#;
        assert_eq!(
            parameter_path(config, "prod/team/api").unwrap(),
            "/prod/api"
        );
        assert!(parameter_path(config, "missing").is_err());
        assert!(parameter_path(config, "bad").is_err());
        assert!(parameter_path("invalid", "prod/team/api").is_err());
    }

    #[test]
    fn diagnoses_workload_without_pods() {
        let mut snapshot: Value = serde_json::from_str(&mock_snapshot()).unwrap();
        snapshot["items"]
            .as_array_mut()
            .unwrap()
            .retain(|v| text(v, "/kind") != "Pod");
        let output = report(&snapshot.to_string(), "api-service").unwrap();
        assert!(output.contains("Deployment api: 0/2 prontos [ATENÇÃO]"));
        assert!(output.contains("Nenhum Pod corresponde"));
    }

    #[test]
    fn mock_query_runs_through_existing_result_handling() {
        let mut app = KubectlUtilApp::new();
        app.load_service_overview();
        assert!(!app.busy);
        app.mock_mode = true;
        app.context.value = "dev-cluster".into();
        app.namespace.value = "dev".into();
        app.service.value = "api-service".into();
        app.load_service_overview();
        assert!(app.busy);
        let result = app
            .rx
            .recv_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        assert_eq!(result.action, Action::ServiceOverview);
        app.handle_result(result);
        assert!(!app.busy);
        assert!(app.output.contains("Deployment api"));
        assert!(app.output.contains("OOMKilled"));
    }
}
