//! Análise local da saída de `kubectl get pods -o wide`.

/// Palavras usadas só quando a saída não tem cabeçalho com a coluna STATUS.
const SUSPICIOUS_WORDS: [&str; 6] = [
    "crashloopbackoff",
    "imagepullbackoff",
    "errimagepull",
    "pending",
    "error",
    "notready",
];

pub fn diagnose_pods(source: &str) -> String {
    let flagged = flagged_lines(source);
    let mut report = String::from("Diagnóstico rápido (análise local da lista de Pods)\n\n");
    if flagged.is_empty() {
        report.push_str("Nenhum estado suspeito foi identificado na saída. Isso não substitui logs/events.\n");
    }
    for line in &flagged {
        report.push_str("[ATENÇÃO] ");
        report.push_str(line);
        report.push('\n');
    }
    report.push_str("\nSaída original:\n");
    report.push_str(source);
    report
}

/// Posição das colunas relevantes, descobertas a partir do cabeçalho.
struct Columns {
    status: usize,
    ready: Option<usize>,
}

fn find_columns(source: &str) -> Option<(usize, Columns)> {
    source.lines().enumerate().find_map(|(line_index, line)| {
        let headers: Vec<&str> = line.split_whitespace().collect();
        let status = headers.iter().position(|h| *h == "STATUS")?;
        let ready = headers.iter().position(|h| *h == "READY");
        Some((line_index, Columns { status, ready }))
    })
}

fn flagged_lines(source: &str) -> Vec<&str> {
    match find_columns(source) {
        Some((header_index, columns)) => source
            .lines()
            .skip(header_index + 1)
            .filter(|line| !line.trim().is_empty())
            .filter(|line| !row_is_healthy(line, &columns))
            .collect(),
        None => source
            .lines()
            .filter(|line| {
                let lower = line.to_ascii_lowercase();
                SUSPICIOUS_WORDS.iter().any(|word| lower.contains(word))
            })
            .collect(),
    }
}

fn row_is_healthy(line: &str, columns: &Columns) -> bool {
    let cells: Vec<&str> = line.split_whitespace().collect();
    let Some(status) = cells.get(columns.status) else {
        // Linha fora do formato esperado: não vale a pena alarmar.
        return true;
    };
    let ready = columns.ready.and_then(|index| cells.get(index).copied());
    match *status {
        "Completed" | "Succeeded" => true,
        "Running" => ready.map_or(true, ready_is_complete),
        _ => false,
    }
}

/// `1/1` e `2/2` estão completos; `0/1` não.
fn ready_is_complete(ready: &str) -> bool {
    match ready.split_once('/') {
        Some((current, total)) => current == total,
        None => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "NAME                          READY   STATUS             RESTARTS   AGE   IP            NODE\n\
api-7d9f6c8b5d-a1b2c           1/1     Running            0          2d    10.0.1.12     mock-node-01\n\
worker-6b8d7f9c4d-x2y3z       0/1     CrashLoopBackOff   5          3h    10.0.2.18     mock-node-02\n\
web-5c6d7e8f9a-m4n5o          1/1     Running            0          1d    10.0.1.25     mock-node-01";

    #[test]
    fn flags_only_unhealthy_rows() {
        let flagged = flagged_lines(SAMPLE);
        assert_eq!(flagged.len(), 1);
        assert!(flagged[0].contains("worker-6b8d7f9c4d-x2y3z"));
    }

    #[test]
    fn pod_name_containing_error_is_not_a_false_positive() {
        let source = "NAME                 READY   STATUS    RESTARTS   AGE\n\
error-handler-abc    1/1     Running   0          1d\n\
pending-jobs-xyz     1/1     Running   0          1d";
        assert!(flagged_lines(source).is_empty());
    }

    #[test]
    fn running_but_not_ready_is_flagged() {
        let source = "NAME    READY   STATUS    RESTARTS   AGE\n\
api-1   0/1     Running   0          1m\n\
api-2   2/2     Running   0          1m";
        let flagged = flagged_lines(source);
        assert_eq!(flagged, vec!["api-1   0/1     Running   0          1m"]);
    }

    #[test]
    fn completed_jobs_are_healthy() {
        let source = "NAME     READY   STATUS      RESTARTS   AGE\n\
job-1    0/1     Completed   0          1h";
        assert!(flagged_lines(source).is_empty());
    }

    #[test]
    fn works_with_namespace_column() {
        let source = "NAMESPACE   NAME    READY   STATUS             RESTARTS   AGE\n\
dev         api-1   1/1     Running            0          1d\n\
dev         api-2   0/1     ImagePullBackOff   0          1d";
        let flagged = flagged_lines(source);
        assert_eq!(flagged.len(), 1);
        assert!(flagged[0].contains("api-2"));
    }

    #[test]
    fn falls_back_to_keywords_without_header() {
        let source = "api-1 Running\nworker-1 CrashLoopBackOff";
        assert_eq!(flagged_lines(source), vec!["worker-1 CrashLoopBackOff"]);
    }

    #[test]
    fn report_mentions_clean_state_and_keeps_original_output() {
        let source = "NAME    READY   STATUS    RESTARTS   AGE\napi-1   1/1     Running   0          1d";
        let report = diagnose_pods(source);
        assert!(report.contains("Nenhum estado suspeito"));
        assert!(report.contains("Saída original:"));
        assert!(report.ends_with(source));
    }

    #[test]
    fn report_marks_flagged_rows() {
        let report = diagnose_pods(SAMPLE);
        assert!(report.contains("[ATENÇÃO] worker-6b8d7f9c4d-x2y3z"));
        assert!(!report.contains("Nenhum estado suspeito"));
    }
}
