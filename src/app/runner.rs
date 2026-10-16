//! Abstração sobre "executar de verdade" ou "simular".
//!
//! O app escolhe o `Runner` uma vez por operação; nenhum outro lugar precisa
//! saber se o Modo Mock está ligado (comandos curtos e port-forward passam
//! pela mesma interface).

use super::mock::{MockScenario, mock_command_result};
use crate::action::Action;
use crate::command::{self, BackgroundProcess, CommandResult};
use std::io;
use std::sync::atomic::AtomicBool;

/// Processo associado a um port-forward ativo.
pub enum Backend {
    Process(BackgroundProcess),
    Simulated,
}

impl Backend {
    pub fn is_simulated(&self) -> bool {
        matches!(self, Self::Simulated)
    }

    /// Descrição do status se o processo já terminou; `None` se continua ativo.
    pub fn exited(&mut self) -> Option<String> {
        match self {
            Self::Process(process) => match process.try_wait() {
                Ok(Some(status)) => Some(status.to_string()),
                Ok(None) => None,
                Err(e) => Some(format!("erro ao consultar o processo: {e}")),
            },
            Self::Simulated => None,
        }
    }

    pub fn stop(&mut self) -> io::Result<()> {
        match self {
            Self::Process(process) => process.stop(),
            Self::Simulated => Ok(()),
        }
    }

    pub fn recent_stderr(&self) -> String {
        match self {
            Self::Process(process) => process.recent_stderr(),
            Self::Simulated => String::new(),
        }
    }
}

pub trait Runner: Send + Sync {
    /// Executa um comando curto e devolve o resultado.
    fn run(
        &self,
        action: Action,
        program: &str,
        args: &[String],
        cancel: &AtomicBool,
    ) -> CommandResult;

    /// Inicia um processo de longa duração. O `Err` já é uma mensagem para o usuário.
    fn start_background(&self, program: &str, args: &[String]) -> Result<Backend, String>;
}

pub struct RealRunner;

impl Runner for RealRunner {
    fn run(
        &self,
        action: Action,
        program: &str,
        args: &[String],
        cancel: &AtomicBool,
    ) -> CommandResult {
        command::run(action, program, args, action.timeout(), cancel)
    }

    fn start_background(&self, program: &str, args: &[String]) -> Result<Backend, String> {
        BackgroundProcess::spawn(program, args)
            .map(Backend::Process)
            .map_err(|e| format!("Falha ao iniciar '{program}': {e}"))
    }
}

pub struct MockRunner {
    pub scenario: MockScenario,
}

impl Runner for MockRunner {
    fn run(
        &self,
        action: Action,
        program: &str,
        args: &[String],
        cancel: &AtomicBool,
    ) -> CommandResult {
        mock_command_result(action, program, args, self.scenario, cancel)
    }

    fn start_background(&self, _program: &str, _args: &[String]) -> Result<Backend, String> {
        match self.scenario {
            MockScenario::Success => Ok(Backend::Simulated),
            other => Err(other.message().to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_runner_starts_simulated_backend_only_on_success() {
        let ok = MockRunner {
            scenario: MockScenario::Success,
        };
        assert!(ok.start_background("kubectl", &[]).unwrap().is_simulated());

        let failing = MockRunner {
            scenario: MockScenario::NoPermission,
        };
        let error = failing.start_background("kubectl", &[]).err().unwrap();
        assert_eq!(error, MockScenario::NoPermission.message());
    }

    #[test]
    fn simulated_backend_never_exits_and_stops_cleanly() {
        let mut backend = Backend::Simulated;
        assert!(backend.exited().is_none());
        assert!(backend.stop().is_ok());
        assert!(backend.recent_stderr().is_empty());
    }

    #[test]
    fn runners_are_object_safe_and_shareable() {
        let runners: Vec<std::sync::Arc<dyn Runner>> = vec![
            std::sync::Arc::new(RealRunner),
            std::sync::Arc::new(MockRunner {
                scenario: MockScenario::Success,
            }),
        ];
        let result = runners[1].run(Action::Namespaces, "kubectl", &[], &AtomicBool::new(false));
        assert!(result.success);
    }

    #[cfg(unix)]
    #[test]
    fn real_runner_reports_spawn_failure_message() {
        let error = RealRunner
            .start_background("programa-que-nao-existe-kubectlutil", &[])
            .err()
            .unwrap();
        assert!(error.starts_with("Falha ao iniciar"));
    }
}
