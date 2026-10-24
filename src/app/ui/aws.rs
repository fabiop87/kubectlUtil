use super::*;
use crate::action::Action;

impl AppView {
    pub(super) fn aws(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        blocks([
            card(
                "Conta, região e cluster",
                "Perfil e região valem para todas as consultas AWS.",
            )
            .child(
                row()
                    .child(self.choice(
                        "profile",
                        "Perfil AWS",
                        |m| &m.profile,
                        |m, v| {
                            if m.profile.value != v {
                                m.profile.value = v;
                                m.params.clear_all();
                            }
                        },
                        window,
                        cx,
                    ))
                    .child(self.button("profiles", "Carregar perfis", cx, |m| m.load_profiles())),
            )
            .child(
                row()
                    .child(self.field(
                        "region",
                        "Região",
                        |m| &m.region,
                        |m, v| {
                            m.region = v;
                            m.params.hide_value();
                        },
                        window,
                        cx,
                    ))
                    .child(self.field(
                        "cluster",
                        "Cluster EKS",
                        |m| &m.cluster,
                        |m, v| m.cluster = v,
                        window,
                        cx,
                    )),
            )
            .child(
                row()
                    .child(self.button("configure", "AWS configure list", cx, |m| {
                        let args = m.aws_args(&["configure", "list"], false);
                        m.aws(Action::AwsConfigure, args);
                    }))
                    .child(self.button("identity", "Identidade STS", cx, |m| {
                        let args = m.aws_args(&["sts", "get-caller-identity"], false);
                        m.aws(Action::AwsIdentity, args);
                    }))
                    .child(self.button("eks", "Listar clusters EKS", cx, |m| {
                        let args = m.aws_args(&["eks", "list-clusters"], true);
                        m.aws(Action::EksClusters, args);
                    }))
                    .child(self.button("kubeconfig", "Atualizar kubeconfig", cx, |m| {
                        let cluster = m.cluster.trim().to_string();
                        if cluster.is_empty() {
                            m.status = "Informe o nome do cluster EKS.".into();
                        } else {
                            let args =
                                m.aws_args(&["eks", "update-kubeconfig", "--name", &cluster], true);
                            m.aws(Action::UpdateKubeconfig, args);
                        }
                    })),
            ),
            card(
                "Autenticação AWS SSO",
                "O login abre o navegador e aguarda autorização por até 5 minutos.",
            )
            .child(
                row()
                    .child(
                        self.button("sso", "Login AWS SSO", cx, |m| m.perform_login())
                            .primary(),
                    )
                    .child(
                        Button::new("copy-sso")
                            .label("Copiar comando SSO")
                            .on_click(cx.listener(|this, _, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(format!(
                                    "aws sso login --profile {}",
                                    this.model.profile.trimmed()
                                )));
                                this.model.status = "Comando SSO copiado.".into();
                                cx.notify();
                            })),
                    ),
            )
            .child(note(
                "Durante o login, use Cancelar no console para interromper.",
            )),
        ])
    }
}
