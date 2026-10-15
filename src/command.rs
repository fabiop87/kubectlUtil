//! Execução de processos externos (kubectl / AWS CLI).
//!
//! - `run`: comando curto, com saída capturada, limite de tempo e cancelamento.
//! - `BackgroundProcess`: processo longo (port-forward) cujo stderr é guardado
//!   (últimas linhas) para explicar por que ele terminou.

use crate::action::Action;
use std::collections::VecDeque;
use std::io::{self, BufRead, BufReader, Read};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub const CANCELLED_MESSAGE: &str = "Operação cancelada pelo usuário.";
const POLL_INTERVAL: Duration = Duration::from_millis(40);
const STDERR_TAIL_LINES: usize = 20;

#[derive(Debug)]
pub struct CommandResult {
    pub action: Action,
    pub output: String,
    pub success: bool,
}

pub fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|s| (*s).to_string()).collect()
}

fn base_command(program: &str, args: &[String]) -> Command {
    let mut command = Command::new(program);
    command.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    command
}

enum Outcome {
    Exited(ExitStatus),
    TimedOut,
    Cancelled,
    WaitFailed(io::Error),
}

/// Executa `program` e espera o fim, o limite de tempo ou o cancelamento.
pub fn run(
    action: Action,
    program: &str,
    args: &[String],
    timeout: Duration,
    cancel: &AtomicBool,
) -> CommandResult {
    let mut command = base_command(program, args);
    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => {
            return CommandResult { action, output: spawn_error_message(program, &e), success: false };
        }
    };

    // Os pipes são lidos em threads para o processo nunca travar com buffer cheio.
    let stdout_reader = child.stdout.take().map(drain);
    let stderr_reader = child.stderr.take().map(drain);

    match wait_for(&mut child, Instant::now() + timeout, cancel) {
        Outcome::Exited(status) => {
            let stdout = collect(stdout_reader);
            let stderr = collect(stderr_reader);
            let success = status.success();
            CommandResult {
                action,
                output: combine_output(&stdout, &stderr, success, &status.to_string()),
                success,
            }
        }
        // Nos casos abaixo as threads de leitura não são aguardadas: elas terminam
        // sozinhas quando os pipes fecharem.
        Outcome::TimedOut => CommandResult {
            action,
            output: format!(
                "Tempo limite de {}s excedido para '{program}'. O processo foi encerrado.",
                timeout.as_secs()
            ),
            success: false,
        },
        Outcome::Cancelled => CommandResult { action, output: CANCELLED_MESSAGE.into(), success: false },
        Outcome::WaitFailed(e) => CommandResult {
            action,
            output: format!("Falha ao aguardar o processo '{program}': {e}"),
            success: false,
        },
    }
}

fn wait_for(child: &mut Child, deadline: Instant, cancel: &AtomicBool) -> Outcome {
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Outcome::Exited(status),
            Ok(None) => {}
            Err(e) => {
                kill_and_reap(child);
                return Outcome::WaitFailed(e);
            }
        }
        if cancel.load(Ordering::Relaxed) {
            kill_and_reap(child);
            return Outcome::Cancelled;
        }
        if Instant::now() >= deadline {
            kill_and_reap(child);
            return Outcome::TimedOut;
        }
        thread::sleep(POLL_INTERVAL);
    }
}

fn kill_and_reap(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn drain<R: Read + Send + 'static>(mut pipe: R) -> JoinHandle<Vec<u8>> {
    thread::spawn(move || {
        let mut buffer = Vec::new();
        let _ = pipe.read_to_end(&mut buffer);
        buffer
    })
}

fn collect(reader: Option<JoinHandle<Vec<u8>>>) -> String {
    reader
        .and_then(|handle| handle.join().ok())
        .map(|bytes| String::from_utf8_lossy(&bytes).trim().to_string())
        .unwrap_or_default()
}

fn spawn_error_message(program: &str, error: &io::Error) -> String {
    format!(
        "Não foi possível executar '{program}'.\n{error}\n\nVerifique se a ferramenta está instalada e disponível no PATH."
    )
}

/// Junta stdout e stderr no texto exibido ao usuário.
pub fn combine_output(stdout: &str, stderr: &str, success: bool, status: &str) -> String {
    match (stdout.is_empty(), stderr.is_empty()) {
        (false, false) => format!("{stdout}\n\n[stderr]\n{stderr}"),
        (false, true) => stdout.to_string(),
        (true, false) => stderr.to_string(),
        (true, true) if success => "Comando concluído sem saída.".into(),
        (true, true) => format!("Comando terminou com status {status}."),
    }
}

/// Processo de longa duração (ex.: `kubectl port-forward`).
///
/// stdout é descartado (o kubectl escreve uma linha por conexão) e as últimas
/// linhas de stderr ficam guardadas para diagnosticar falhas. O processo é
/// encerrado quando o valor é descartado.
pub struct BackgroundProcess {
    child: Child,
    stderr_tail: Arc<Mutex<VecDeque<String>>>,
    stderr_reader: Option<JoinHandle<()>>,
}

impl BackgroundProcess {
    pub fn spawn(program: &str, args: &[String]) -> io::Result<Self> {
        let mut command = base_command(program, args);
        command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = command.spawn()?;

        if let Some(mut stdout) = child.stdout.take() {
            thread::spawn(move || {
                let _ = io::copy(&mut stdout, &mut io::sink());
            });
        }

        let stderr_tail = Arc::new(Mutex::new(VecDeque::new()));
        let stderr_reader = child.stderr.take().map(|stderr| {
            let tail = Arc::clone(&stderr_tail);
            thread::spawn(move || {
                for line in BufReader::new(stderr).split(b'\n').map_while(Result::ok) {
                    let line = String::from_utf8_lossy(&line).trim().to_string();
                    if line.is_empty() {
                        continue;
                    }
                    if let Ok(mut tail) = tail.lock() {
                        if tail.len() == STDERR_TAIL_LINES {
                            tail.pop_front();
                        }
                        tail.push_back(line);
                    }
                }
            })
        });

        Ok(Self { child, stderr_tail, stderr_reader })
    }

    /// `Ok(Some(status))` quando o processo já terminou.
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        let status = self.child.try_wait()?;
        if status.is_some() {
            self.settle_stderr();
        }
        Ok(status)
    }

    /// Encerra o processo (sem erro se ele já tinha terminado).
    pub fn stop(&mut self) -> io::Result<()> {
        if self.child.try_wait()?.is_some() {
            return Ok(());
        }
        self.child.kill()?;
        let _ = self.child.wait();
        Ok(())
    }

    /// Últimas linhas escritas no stderr.
    pub fn recent_stderr(&self) -> String {
        match self.stderr_tail.lock() {
            Ok(tail) => tail.iter().cloned().collect::<Vec<_>>().join("\n"),
            Err(_) => String::new(),
        }
    }

    /// Dá um instante para a thread de leitura consumir o que o processo escreveu
    /// antes de sair; sem isso a última linha de erro poderia ser perdida.
    fn settle_stderr(&mut self) {
        if let Some(handle) = self.stderr_reader.take() {
            for _ in 0..20 {
                if handle.is_finished() {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
            if handle.is_finished() {
                let _ = handle.join();
            }
        }
    }
}

impl Drop for BackgroundProcess {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combine_output_covers_all_cases() {
        assert_eq!(combine_output("out", "err", true, "0"), "out\n\n[stderr]\nerr");
        assert_eq!(combine_output("out", "", true, "0"), "out");
        assert_eq!(combine_output("", "err", false, "1"), "err");
        assert_eq!(combine_output("", "", true, "0"), "Comando concluído sem saída.");
        assert_eq!(combine_output("", "", false, "1"), "Comando terminou com status 1.");
    }

    #[test]
    fn args_converts_to_owned_strings() {
        assert_eq!(args(&["get", "pods"]), vec!["get".to_string(), "pods".to_string()]);
    }

    #[test]
    fn missing_program_reports_path_hint() {
        let cancel = AtomicBool::new(false);
        let result = run(
            Action::Nodes,
            "programa-que-nao-existe-kubectlutil",
            &[],
            Duration::from_secs(5),
            &cancel,
        );
        assert!(!result.success);
        assert!(result.output.contains("PATH"));
    }

    #[cfg(unix)]
    mod unix {
        use super::*;

        fn sh(script: &str) -> Vec<String> {
            args(&["-c", script])
        }

        #[test]
        fn run_captures_stdout() {
            let cancel = AtomicBool::new(false);
            let result = run(Action::Nodes, "sh", &sh("echo ola"), Duration::from_secs(5), &cancel);
            assert!(result.success);
            assert_eq!(result.output, "ola");
        }

        #[test]
        fn run_reports_failure_with_stderr() {
            let cancel = AtomicBool::new(false);
            let result = run(Action::Nodes, "sh", &sh("echo falhou >&2; exit 3"), Duration::from_secs(5), &cancel);
            assert!(!result.success);
            assert!(result.output.contains("falhou"));
        }

        #[test]
        fn run_enforces_timeout() {
            let cancel = AtomicBool::new(false);
            let started = Instant::now();
            let result = run(Action::Nodes, "sh", &sh("sleep 10"), Duration::from_millis(200), &cancel);
            assert!(!result.success);
            assert!(result.output.contains("Tempo limite"));
            assert!(started.elapsed() < Duration::from_secs(5));
        }

        #[test]
        fn run_honors_cancel_flag() {
            let cancel = Arc::new(AtomicBool::new(false));
            let flag = Arc::clone(&cancel);
            let canceller = thread::spawn(move || {
                thread::sleep(Duration::from_millis(150));
                flag.store(true, Ordering::Relaxed);
            });
            let result = run(Action::Nodes, "sh", &sh("sleep 10"), Duration::from_secs(30), &cancel);
            canceller.join().unwrap();
            assert!(!result.success);
            assert_eq!(result.output, CANCELLED_MESSAGE);
        }

        #[test]
        fn background_process_exposes_exit_and_stderr() {
            let mut process = BackgroundProcess::spawn("sh", &sh("echo erro-de-porta >&2; exit 3")).unwrap();
            let mut status = None;
            for _ in 0..100 {
                status = process.try_wait().unwrap();
                if status.is_some() {
                    break;
                }
                thread::sleep(Duration::from_millis(20));
            }
            assert!(status.is_some());
            assert!(!status.unwrap().success());
            assert!(process.recent_stderr().contains("erro-de-porta"));
        }

        #[test]
        fn background_process_can_be_stopped() {
            let mut process = BackgroundProcess::spawn("sh", &sh("sleep 30")).unwrap();
            assert!(process.try_wait().unwrap().is_none());
            process.stop().unwrap();
            assert!(process.try_wait().unwrap().is_some());
            // Parar de novo não deve falhar.
            process.stop().unwrap();
        }
    }
}
