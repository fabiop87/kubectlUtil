//! Lista de `kubectl port-forward` ativos e verificação de processos que terminaram.

use super::runner::Backend;
use std::io;

pub struct ActivePortForward {
    pub target: String,
    pub namespace: String,
    pub local_port: String,
    pub remote_port: String,
    pub backend: Backend,
}

/// Port-forward cujo processo terminou sozinho (Pod reiniciou, porta ocupada, rede caiu…).
pub struct EndedForward {
    pub target: String,
    pub namespace: String,
    pub local_port: String,
    pub remote_port: String,
    pub status: String,
    pub stderr: String,
}

pub enum StopOutcome {
    NotFound,
    Stopped(ActivePortForward),
    /// Não foi possível encerrar; o item continua na lista.
    Failed(String, io::Error),
}

#[derive(Default)]
pub struct PortForwardManager {
    items: Vec<ActivePortForward>,
}

impl PortForwardManager {
    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &ActivePortForward> {
        self.items.iter()
    }

    pub fn push(&mut self, item: ActivePortForward) {
        self.items.push(item);
    }

    pub fn local_port_in_use(&self, port: &str) -> bool {
        self.items.iter().any(|pf| pf.local_port == port)
    }

    /// Existe algum processo real que precisa ser vigiado?
    pub fn has_processes(&self) -> bool {
        self.items.iter().any(|pf| !pf.backend.is_simulated())
    }

    pub fn stop(&mut self, index: usize) -> StopOutcome {
        let Some(item) = self.items.get_mut(index) else {
            return StopOutcome::NotFound;
        };
        match item.backend.stop() {
            Ok(()) => StopOutcome::Stopped(self.items.remove(index)),
            Err(e) => StopOutcome::Failed(item.target.clone(), e),
        }
    }

    pub fn stop_all(&mut self) {
        for mut item in self.items.drain(..) {
            let _ = item.backend.stop();
        }
    }

    /// Remove os port-forwards simulados (ao sair do Modo Mock).
    pub fn drop_simulated(&mut self) {
        self.items.retain(|pf| !pf.backend.is_simulated());
    }

    /// Remove da lista e devolve os port-forwards cujo processo já terminou.
    pub fn poll_exited(&mut self) -> Vec<EndedForward> {
        let mut ended = Vec::new();
        let mut index = 0;
        while index < self.items.len() {
            match self.items[index].backend.exited() {
                Some(status) => {
                    let item = self.items.remove(index);
                    ended.push(EndedForward {
                        stderr: item.backend.recent_stderr(),
                        status,
                        target: item.target,
                        namespace: item.namespace,
                        local_port: item.local_port,
                        remote_port: item.remote_port,
                    });
                }
                None => index += 1,
            }
        }
        ended
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simulated(local_port: &str) -> ActivePortForward {
        ActivePortForward {
            target: "svc/db".into(),
            namespace: "dev".into(),
            local_port: local_port.into(),
            remote_port: "5432".into(),
            backend: Backend::Simulated,
        }
    }

    #[test]
    fn detects_local_port_in_use() {
        let mut manager = PortForwardManager::default();
        assert!(manager.is_empty());
        manager.push(simulated("15432"));
        assert_eq!(manager.len(), 1);
        assert!(manager.local_port_in_use("15432"));
        assert!(!manager.local_port_in_use("16379"));
    }

    #[test]
    fn stop_removes_item_or_reports_not_found() {
        let mut manager = PortForwardManager::default();
        manager.push(simulated("1"));
        manager.push(simulated("2"));
        assert!(matches!(manager.stop(5), StopOutcome::NotFound));
        match manager.stop(0) {
            StopOutcome::Stopped(pf) => assert_eq!(pf.local_port, "1"),
            _ => panic!("esperava Stopped"),
        }
        assert_eq!(manager.len(), 1);
        assert!(manager.local_port_in_use("2"));
    }

    #[test]
    fn simulated_items_are_not_watched_and_can_be_dropped() {
        let mut manager = PortForwardManager::default();
        manager.push(simulated("1"));
        assert!(!manager.has_processes());
        assert!(manager.poll_exited().is_empty());
        manager.drop_simulated();
        assert!(manager.is_empty());
    }

    #[test]
    fn stop_all_empties_the_list() {
        let mut manager = PortForwardManager::default();
        manager.push(simulated("1"));
        manager.push(simulated("2"));
        manager.stop_all();
        assert!(manager.is_empty());
    }

    #[cfg(unix)]
    mod unix {
        use super::*;
        use crate::command::{BackgroundProcess, args};
        use std::thread;
        use std::time::Duration;

        fn real(script: &str, local_port: &str) -> ActivePortForward {
            let process = BackgroundProcess::spawn("sh", &args(&["-c", script])).unwrap();
            ActivePortForward {
                target: "pod/api".into(),
                namespace: "dev".into(),
                local_port: local_port.into(),
                remote_port: "8080".into(),
                backend: Backend::Process(process),
            }
        }

        #[test]
        fn poll_exited_reports_dead_process_with_stderr() {
            let mut manager = PortForwardManager::default();
            manager.push(real(
                "echo 'unable to listen on port 5434' >&2; exit 1",
                "5434",
            ));
            manager.push(real("sleep 30", "5435"));
            assert!(manager.has_processes());

            let mut ended = Vec::new();
            for _ in 0..100 {
                ended = manager.poll_exited();
                if !ended.is_empty() {
                    break;
                }
                thread::sleep(Duration::from_millis(20));
            }

            assert_eq!(ended.len(), 1);
            assert_eq!(ended[0].local_port, "5434");
            assert!(ended[0].stderr.contains("unable to listen"));
            // O processo que continua vivo permanece na lista.
            assert_eq!(manager.len(), 1);
            assert!(manager.local_port_in_use("5435"));
            manager.stop_all();
        }

        #[test]
        fn stop_kills_a_live_process() {
            let mut manager = PortForwardManager::default();
            manager.push(real("sleep 30", "5436"));
            assert!(matches!(manager.stop(0), StopOutcome::Stopped(_)));
            assert!(manager.is_empty());
        }
    }
}
