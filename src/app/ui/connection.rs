use super::*;
use crate::app::presets;

impl AppView {
    pub(super) fn connection(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let selectors = card(
            "Conexão Kubernetes",
            "O contexto selecionado é enviado em cada consulta. Carregue as opções sob demanda.",
        )
        .child(
            row()
                .child(self.choice(
                    "context",
                    "Contexto",
                    |m| &m.context,
                    |m, value| {
                        if m.context.value != value {
                            m.context.value = value;
                            m.on_context_changed();
                        }
                    },
                    window,
                    cx,
                ))
                .child(self.button("contexts", "Atualizar contextos", cx, |m| {
                    m.refresh_contexts()
                }))
                .child(
                    self.button("current-context", "Identificar contexto ativo", cx, |m| {
                        m.fetch_current_context()
                    }),
                )
                .child(
                    self.button("use-context", "Gravar no kubeconfig", cx, |m| {
                        m.use_selected_context()
                    })
                    .tooltip("Altera o contexto padrão dos seus outros terminais."),
                ),
        )
        .child(
            row()
                .child(self.choice(
                    "namespace",
                    "Namespace",
                    |m| &m.namespace,
                    |m, value| {
                        if m.namespace.value != value {
                            m.namespace.value = value;
                            m.pod = Default::default();
                            m.service = Default::default();
                            m.explorer_output.clear();
                            m.explorer_namespace.clear();
                        }
                    },
                    window,
                    cx,
                ))
                .child(self.button("namespaces", "Carregar namespaces", cx, |m| {
                    m.refresh_namespaces()
                }))
                .child(self.button("pods", "Carregar Pods", cx, |m| m.load_pods()))
                .child(self.button("services", "Carregar Services", cx, |m| m.load_services())),
        )
        .child(
            row()
                .child(self.choice("pod", "Pod", |m| &m.pod, |m, v| m.pod.value = v, window, cx))
                .child(self.choice(
                    "service",
                    "Service",
                    |m| &m.service,
                    |m, v| m.service.value = v,
                    window,
                    cx,
                )),
        );
        let mut forward = card(
            "Port Forward",
            "Cada encaminhamento usa uma porta local exclusiva.",
        )
        .child(
            row()
                .child(self.field(
                    "local-port",
                    "Porta local",
                    |m| &m.local_port,
                    |m, v| m.local_port = v,
                    window,
                    cx,
                ))
                .child(self.field(
                    "remote-port",
                    "Porta remota",
                    |m| &m.remote_port,
                    |m, v| m.remote_port = v,
                    window,
                    cx,
                )),
        )
        .child(
            row()
                .child(
                    self.button("forward-pod", "Encaminhar Pod", cx, |m| {
                        let pod = m.pod.trimmed().to_string();
                        if pod.is_empty() {
                            m.status = "Selecione um Pod.".into();
                        } else {
                            m.start_port_forward(pod);
                        }
                    })
                    .primary(),
                )
                .child(
                    self.button("forward-service", "Encaminhar Service", cx, |m| {
                        let service = m.service.trimmed().to_string();
                        if service.is_empty() {
                            m.status = "Selecione um Service.".into();
                        } else {
                            m.start_port_forward(format!("svc/{service}"));
                        }
                    }),
                ),
        );
        for (index, pf) in self.model.port_forwards.iter().enumerate() {
            forward = forward.child(
                row()
                    .child(div().text_color(rgb(0x8CE2C1)).child(format!(
                        "● localhost:{} → {} · {} · {}{}",
                        pf.local_port,
                        pf.remote_port,
                        pf.target,
                        pf.namespace,
                        if pf.backend.is_simulated() {
                            " · MOCK"
                        } else {
                            ""
                        }
                    )))
                    .child(
                        Button::new(("stop-forward", index))
                            .label("Parar")
                            .danger()
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.model.stop_port_forward(index);
                                cx.notify();
                            })),
                    ),
            );
        }
        if !self.model.port_forwards.is_empty() {
            forward = forward.child(
                Button::new("stop-all")
                    .label("Parar todos")
                    .danger()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.model.stop_all_port_forwards();
                        cx.notify();
                    })),
            );
        }
        let needle = self.model.filter.to_lowercase();
        let explorer = card(
            "Explorador e diagnóstico",
            "A última listagem alimenta o filtro e o diagnóstico local.",
        )
        .child(
            row()
                .child(self.button("explorer", "Carregar Pods", cx, |m| m.load_pod_explorer()))
                .child(self.button("diagnose", "Diagnóstico rápido", cx, |m| {
                    m.run_pod_diagnostics()
                })),
        )
        .child(self.field(
            "pod-filter",
            "Filtrar lista carregada",
            |m| &m.filter,
            |m, v| m.filter = v,
            window,
            cx,
        ))
        .child(
            div()
                .id("explorer-lines")
                .max_h(px(160.))
                .overflow_scroll()
                .font_family("Consolas")
                .children(
                    self.model
                        .explorer_output
                        .lines()
                        .filter(|line| line.to_lowercase().contains(&needle))
                        .map(|line| div().child(line.to_string())),
                ),
        );
        div()
            .flex()
            .flex_col()
            .gap_4()
            .child(blocks([selectors, forward]))
            .child(blocks([
                explorer,
                card("Troubleshooting", "Ferramentas para o Pod selecionado.")
                    .child(self.preset_buttons(presets::POD_TOOLS, cx))
                    .child(self.button("exec", "Bash no Pod", cx, |m| m.show_exec_hint())),
            ]))
    }
}

impl KubectlUtilApp {
    fn show_exec_hint(&mut self) {
        let namespace = self.namespace.trimmed();
        let pod = self.pod.trimmed();
        if namespace.is_empty() || pod.is_empty() {
            self.status = "Selecione namespace e Pod.".into();
            return;
        }
        let context = self.context.trimmed();
        let flag = if context.is_empty() {
            String::new()
        } else {
            format!(" --context {context}")
        };
        self.status = "Bash interativo requer terminal externo; use o comando abaixo.".into();
        self.output_title = "Bash no Pod".into();
        self.output = format!(
            "kubectl exec -it{flag} -n {namespace} {pod} -- bash\n\nExecute em um terminal separado."
        );
    }
}
