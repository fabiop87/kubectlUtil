//! Catálogo declarativo de consultas `kubectl`.
//!
//! Para adicionar um botão, basta incluir uma linha em uma das listas abaixo
//! (e, se for uma operação nova, uma variante em `Action` com seus dados no mock).
//! Os argumentos aceitam os marcadores `{ns}` (namespace) e `{pod}` (Pod selecionado).

use crate::action::Action;

/// O que precisa estar selecionado na interface antes de executar o comando.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Needs {
    Nothing,
    Namespace,
    NamespaceAndPod,
}

impl Needs {
    /// Mensagem exibida quando falta alguma seleção.
    pub fn missing_message(self) -> &'static str {
        match self {
            Self::Nothing => "",
            Self::Namespace => "Selecione um namespace.",
            Self::NamespaceAndPod => "Selecione namespace e Pod.",
        }
    }

    pub fn is_satisfied(self, namespace: &str, pod: &str) -> bool {
        match self {
            Self::Nothing => true,
            Self::Namespace => !namespace.is_empty(),
            Self::NamespaceAndPod => !namespace.is_empty() && !pod.is_empty(),
        }
    }
}

pub struct Preset {
    pub label: &'static str,
    pub action: Action,
    pub needs: Needs,
    pub args: &'static [&'static str],
}

impl Preset {
    pub fn build_args(&self, namespace: &str, pod: &str) -> Vec<String> {
        self.args
            .iter()
            .map(|arg| arg.replace("{ns}", namespace).replace("{pod}", pod))
            .collect()
    }
}

const CLUSTER_INFO: Preset = Preset {
    label: "Cluster info",
    action: Action::ClusterInfo,
    needs: Needs::Nothing,
    args: &["cluster-info"],
};

const NODES: Preset = Preset {
    label: "Nodes (wide)",
    action: Action::Nodes,
    needs: Needs::Nothing,
    args: &["get", "nodes", "-o", "wide"],
};

const TOP_NODES: Preset = Preset {
    label: "Top Nodes",
    action: Action::TopNodes,
    needs: Needs::Nothing,
    args: &["top", "nodes"],
};

const WORKLOADS: Preset = Preset {
    label: "Workloads",
    action: Action::Workloads,
    needs: Needs::Namespace,
    args: &["get", "deployments,daemonsets,statefulsets,jobs", "-n", "{ns}"],
};

const EVENTS: Preset = Preset {
    label: "Eventos",
    action: Action::Events,
    needs: Needs::Namespace,
    args: &["get", "events", "-n", "{ns}", "--sort-by=.metadata.creationTimestamp"],
};

const SERVICES: Preset = Preset {
    label: "Services",
    action: Action::ServicesTable,
    needs: Needs::Namespace,
    args: &["get", "svc", "-n", "{ns}"],
};

const IMAGES: Preset = Preset {
    label: "Imagens dos Pods",
    action: Action::Images,
    needs: Needs::Namespace,
    args: &[
        "get",
        "pods",
        "-n",
        "{ns}",
        "-o",
        "jsonpath={range .items[*]}{.metadata.name}{\"\\t\"}{range .spec.containers[*]}{.image}{\" \"}{end}{\"\\n\"}{end}",
    ],
};

/// Atalhos da aba "Visão geral".
pub const OVERVIEW: &[Preset] = &[CLUSTER_INFO, NODES, EVENTS];

/// Aba "Cluster".
pub const CLUSTER: &[Preset] = &[NODES, TOP_NODES, WORKLOADS, CLUSTER_INFO, EVENTS, SERVICES, IMAGES];

/// Aba "Rede e storage".
pub const NETWORK: &[Preset] = &[
    Preset {
        label: "Ingresses",
        action: Action::Ingresses,
        needs: Needs::Namespace,
        args: &["get", "ingress", "-n", "{ns}"],
    },
    Preset {
        label: "Detalhes de Ingress",
        action: Action::IngressDetails,
        needs: Needs::Namespace,
        args: &["describe", "ingress", "-n", "{ns}"],
    },
    Preset {
        label: "Endpoints",
        action: Action::Endpoints,
        needs: Needs::Namespace,
        args: &["get", "endpoints", "-n", "{ns}"],
    },
    Preset {
        label: "PVC e PV",
        action: Action::PvcAndPv,
        needs: Needs::Namespace,
        args: &["get", "pvc,pv", "-n", "{ns}"],
    },
    Preset {
        label: "ConfigMaps",
        action: Action::ConfigMaps,
        needs: Needs::Namespace,
        args: &["get", "configmaps", "-n", "{ns}"],
    },
    Preset {
        label: "Backups Velero",
        action: Action::VeleroBackups,
        needs: Needs::Namespace,
        args: &["get", "backups", "-n", "{ns}"],
    },
];

/// Ferramentas do Pod selecionado (aba "Conexão Kubernetes").
pub const POD_TOOLS: &[Preset] = &[
    Preset {
        label: "Logs (últimas 100 linhas)",
        action: Action::PodLogs,
        needs: Needs::NamespaceAndPod,
        args: &["logs", "-n", "{ns}", "{pod}", "--tail=100"],
    },
    Preset {
        label: "Logs anteriores (--previous)",
        action: Action::PodLogsPrevious,
        needs: Needs::NamespaceAndPod,
        args: &["logs", "-n", "{ns}", "{pod}", "--previous", "--tail=100"],
    },
    Preset {
        label: "Descrever Pod",
        action: Action::DescribePod,
        needs: Needs::NamespaceAndPod,
        args: &["describe", "pod", "-n", "{ns}", "{pod}"],
    },
    Preset {
        label: "YAML do Pod",
        action: Action::PodYaml,
        needs: Needs::NamespaceAndPod,
        args: &["get", "pod", "-n", "{ns}", "{pod}", "-o", "yaml"],
    },
    Preset {
        label: "Top Pods",
        action: Action::TopPods,
        needs: Needs::Namespace,
        args: &["top", "pods", "-n", "{ns}"],
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placeholders_are_replaced() {
        let preset = &POD_TOOLS[0];
        assert_eq!(
            preset.build_args("dev", "api-1"),
            vec!["logs", "-n", "dev", "api-1", "--tail=100"]
        );
    }

    #[test]
    fn jsonpath_braces_are_not_touched() {
        let args = IMAGES.build_args("dev", "");
        assert_eq!(args[3], "dev");
        assert!(args[5].contains("{range .items[*]}{.metadata.name}"));
        assert!(args[5].contains("{.image}"));
    }

    #[test]
    fn needs_checks_selection() {
        assert!(Needs::Nothing.is_satisfied("", ""));
        assert!(!Needs::Namespace.is_satisfied("", "pod"));
        assert!(Needs::Namespace.is_satisfied("dev", ""));
        assert!(!Needs::NamespaceAndPod.is_satisfied("dev", ""));
        assert!(Needs::NamespaceAndPod.is_satisfied("dev", "pod"));
    }

    #[test]
    fn every_preset_that_needs_a_namespace_uses_the_placeholder() {
        for list in [OVERVIEW, CLUSTER, NETWORK, POD_TOOLS] {
            for preset in list {
                let uses_ns = preset.args.iter().any(|a| a.contains("{ns}"));
                assert_eq!(
                    preset.needs != Needs::Nothing,
                    uses_ns,
                    "preset '{}' inconsistente entre `needs` e `args`",
                    preset.label
                );
            }
        }
    }

    #[test]
    fn labels_are_unique_within_a_list() {
        for list in [OVERVIEW, CLUSTER, NETWORK, POD_TOOLS] {
            let mut labels: Vec<&str> = list.iter().map(|p| p.label).collect();
            let total = labels.len();
            labels.sort_unstable();
            labels.dedup();
            assert_eq!(labels.len(), total);
        }
    }
}
