//! O que fazer quando um comando termina: atualizar listas, painel de saída,
//! histórico e o estado do Parameter Store.

use super::KubectlUtilApp;
use super::diagnostics::diagnose_pods;
use super::parameters::{is_invalid_credentials, parse_parameters};
use super::util::timestamp;
use crate::action::Action;
use crate::command::CommandResult;

impl KubectlUtilApp {
    pub(super) fn handle_result(&mut self, result: CommandResult) {
        self.busy = false;

        // Consulta auxiliar (contexto atual): não mexe no painel de saída nem no histórico.
        if result.action.is_silent() {
            self.status = "Pronto".into();
            if result.success && self.context.value.trim().is_empty() {
                let name = result.output.trim();
                if !name.is_empty() {
                    self.context.value = name.to_string();
                }
            }
            return;
        }

        self.status = if result.success {
            "Concluído".into()
        } else {
            "Com erro — veja a saída".into()
        };

        if result.action.is_secret() {
            self.handle_secret_result(result);
            return;
        }

        self.output_title = result.action.title().into();
        self.output = result.output.clone();
        let outcome = if result.success { "OK" } else { "ERRO" };
        self.log_history(format!(
            "{} | {} | {}",
            timestamp(),
            result.action.title(),
            outcome
        ));

        if result.success {
            self.apply_success(&result);
        } else if result.action == Action::SsmList && is_invalid_credentials(&result.output) {
            self.status = format!(
                "Credenciais AWS inválidas ou expiradas para o perfil '{profile}'. Atualize a sessão e tente novamente. Se usa AWS SSO, use o botão de login na aba AWS & EKS (ou execute `aws sso login --profile {profile}` no terminal); se usa credenciais temporárias, renove também AWS_SESSION_TOKEN. Confira o perfil e a região.",
                profile = self.profile.value
            );
        }
    }

    /// Valores de parâmetros nunca vão ao painel geral nem ao histórico.
    fn handle_secret_result(&mut self, result: CommandResult) {
        let ok = result.success;
        if ok && self.section == super::Section::Parameters {
            self.params.set_revealed(result.output.clone());
        } else {
            self.params.hide_value();
        }
        self.output_title = "Parameter Store — resultado seguro".into();
        self.output = if ok {
            "Valor carregado na área segura do Parameter Store. O conteúdo não foi adicionado ao histórico nem ao log geral.".into()
        } else {
            result.output
        };
        let outcome = if ok { "OK" } else { "ERRO" };
        self.log_history(format!(
            "{} | consulta de valor de parâmetro | {}",
            timestamp(),
            outcome
        ));
    }

    /// Atualiza listas e estados a partir de um resultado bem-sucedido.
    ///
    /// Em caso de erro as listas são preservadas: a mensagem de erro aparece no
    /// painel de saída e não vira item de combo.
    fn apply_success(&mut self, result: &CommandResult) {
        match result.action {
            Action::Contexts => {
                self.context.items = non_empty_lines(&result.output);
                if self.context.value.trim().is_empty() {
                    self.fetch_current_context();
                }
            }
            Action::Namespaces => {
                self.namespace.items = non_empty_lines(&result.output);
                if !self.namespace.items.contains(&self.namespace.value) {
                    self.namespace.value.clear();
                }
            }
            Action::PodNames => {
                self.pod.items = non_empty_lines(&result.output);
                self.pod.value.clear();
            }
            Action::ServiceNames => {
                self.service.items = non_empty_lines(&result.output);
                self.service.value.clear();
            }
            Action::AwsProfiles => {
                self.profile.items = non_empty_lines(&result.output);
                if !self.profile.items.is_empty()
                    && !self.profile.items.contains(&self.profile.value)
                {
                    self.profile.value = self.profile.items[0].clone();
                }
            }
            Action::PodExplorer => {
                self.explorer_output = result.output.clone();
            }
            Action::PodDiagnostics => {
                self.output = diagnose_pods(&result.output);
                self.explorer_output = result.output.clone();
            }
            Action::SsmList => match parse_parameters(&result.output) {
                Ok(entries) => {
                    let total = entries.len();
                    self.params.store_loaded(entries);
                    self.status = format!(
                        "{total} parâmetros carregados. Valores SecureString não foram descriptografados."
                    );
                }
                Err(e) => {
                    self.status =
                        format!("Não foi possível interpretar o JSON do Parameter Store: {e}");
                }
            },
            _ => {}
        }
    }
}

/// Linhas aparadas e sem vazios (formato das listas de nomes do kubectl/aws).
fn non_empty_lines(output: &str) -> Vec<String> {
    output
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_empty_lines_trims_and_skips_blanks() {
        assert_eq!(non_empty_lines("  a \n\n b\n   \n"), vec!["a", "b"]);
        assert!(non_empty_lines("").is_empty());
    }
}
