//! Identifica cada operação que o aplicativo pode executar.
//!
//! Antes, o "título" (string) servia de chave para tudo: botões, tratamento de
//! resultado e dados simulados. Com um enum, o compilador avisa quando uma
//! operação nova não foi tratada em algum lugar (por exemplo, no mock).

use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    // Kubernetes: conexão e seleção
    Contexts,
    CurrentContext,
    UseContext,
    Namespaces,
    PodNames,
    ServiceNames,
    // Kubernetes: consultas
    PodExplorer,
    PodDiagnostics,
    PodLogs,
    PodLogsPrevious,
    DescribePod,
    PodYaml,
    TopPods,
    ClusterInfo,
    Nodes,
    TopNodes,
    Events,
    Workloads,
    ServicesTable,
    Images,
    Ingresses,
    IngressDetails,
    Endpoints,
    PvcAndPv,
    ConfigMaps,
    VeleroBackups,
    // AWS
    AwsProfiles,
    AwsConfigure,
    AwsIdentity,
    EksClusters,
    UpdateKubeconfig,
    AwsSsoLogin,
    // Parameter Store
    SsmList,
    SsmRevealSecure,
    SsmRevealPlain,
}

impl Action {
    /// Texto exibido no painel de saída, no status e no histórico.
    pub fn title(self) -> &'static str {
        match self {
            Self::Contexts => "Contextos Kubernetes",
            Self::CurrentContext => "Contexto atual",
            Self::UseContext => "Alterar contexto",
            Self::Namespaces => "Namespaces",
            Self::PodNames => "Pods",
            Self::ServiceNames => "Services",
            Self::PodExplorer => "Explorador de Pods",
            Self::PodDiagnostics => "Diagnóstico de Pods",
            Self::PodLogs => "Logs do Pod",
            Self::PodLogsPrevious => "Logs anteriores do Pod",
            Self::DescribePod => "Describe Pod",
            Self::PodYaml => "YAML do Pod",
            Self::TopPods => "Top Pods",
            Self::ClusterInfo => "Informações do cluster",
            Self::Nodes => "Nodes",
            Self::TopNodes => "Top Nodes",
            Self::Events => "Eventos",
            Self::Workloads => "Workloads",
            Self::ServicesTable => "Services (detalhes)",
            Self::Images => "Imagens",
            Self::Ingresses => "Ingresses",
            Self::IngressDetails => "Detalhes de Ingress",
            Self::Endpoints => "Endpoints",
            Self::PvcAndPv => "PVC e PV",
            Self::ConfigMaps => "ConfigMaps",
            Self::VeleroBackups => "Backups Velero",
            Self::AwsProfiles => "Perfis AWS",
            Self::AwsConfigure => "Configuração AWS",
            Self::AwsIdentity => "Identidade STS",
            Self::EksClusters => "Clusters EKS",
            Self::UpdateKubeconfig => "Atualizar kubeconfig",
            Self::AwsSsoLogin => "Login AWS SSO",
            Self::SsmList => "Listar parâmetros SSM",
            Self::SsmRevealSecure => "Revelar SecureString",
            Self::SsmRevealPlain => "Revelar parâmetro",
        }
    }

    /// O resultado contém um valor sensível: nunca vai ao painel geral nem ao histórico.
    pub fn is_secret(self) -> bool {
        matches!(self, Self::SsmRevealSecure | Self::SsmRevealPlain)
    }

    /// Consulta auxiliar que não deve sobrescrever o painel de saída.
    pub fn is_silent(self) -> bool {
        matches!(self, Self::CurrentContext)
    }

    /// Tempo máximo antes de o processo ser encerrado.
    pub fn timeout(self) -> Duration {
        match self {
            // O login SSO espera o usuário autorizar no navegador.
            Self::AwsSsoLogin => Duration::from_secs(300),
            _ => Duration::from_secs(60),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_reveal_actions_are_secret() {
        assert!(Action::SsmRevealSecure.is_secret());
        assert!(Action::SsmRevealPlain.is_secret());
        assert!(!Action::SsmList.is_secret());
        assert!(!Action::PodLogs.is_secret());
    }

    #[test]
    fn sso_login_has_longer_timeout() {
        assert!(Action::AwsSsoLogin.timeout() > Action::Nodes.timeout());
    }

    #[test]
    fn titles_are_not_empty() {
        assert!(!Action::Contexts.title().is_empty());
        assert_ne!(Action::PodNames.title(), Action::ServicesTable.title());
    }
}
