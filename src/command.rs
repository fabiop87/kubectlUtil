use std::process::{Child, Command, Stdio};

#[derive(Debug)]
pub struct CommandResult {
    pub title: String,
    pub output: String,
    pub success: bool,
}

pub fn run(title: impl Into<String>, program: &str, args: &[String]) -> CommandResult {
    let title = title.into();
    let mut command = Command::new(program);
    command.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    match command.output() {
        Ok(result) => {
            let stdout = String::from_utf8_lossy(&result.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&result.stderr).trim().to_string();
            let output = match (stdout.is_empty(), stderr.is_empty()) {
                (false, false) => format!("{stdout}\n\n[stderr]\n{stderr}"),
                (false, true) => stdout,
                (true, false) => stderr,
                (true, true) if result.status.success() => "Comando concluído sem saída.".into(),
                (true, true) => format!("Comando terminou com status {}.", result.status),
            };
            CommandResult { title, output, success: result.status.success() }
        }
        Err(e) => CommandResult {
            title,
            output: format!("Não foi possível executar '{program}'.\n{e}\n\nVerifique se a ferramenta está instalada e disponível no PATH."),
            success: false,
        },
    }
}

pub fn args(values: &[&str]) -> Vec<String> {
    values.iter().map(|s| (*s).to_string()).collect()
}


/// Starts a long-running command without capturing pipes, so it cannot block on full buffers.
pub fn spawn(program: &str, args: &[String]) -> std::io::Result<Child> {
    let mut command = Command::new(program);
    command.args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    command.spawn()
}
