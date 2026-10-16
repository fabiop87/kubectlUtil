//! Aba "AWS & EKS".

use super::widgets::{card, field_combo, palette_of, primary_button};
use crate::action::Action;
use crate::app::KubectlUtilApp;
use eframe::egui;

impl KubectlUtilApp {
    pub(super) fn show_aws(&mut self, ui: &mut egui::Ui) {
        self.show_aws_account(ui);
        //ui.add_space(12.0);
        self.show_aws_login(ui);
    }

    fn show_aws_account(&mut self, ui: &mut egui::Ui) {
        card(
            ui,
            "Conta, região e cluster",
            Some("Perfil e região valem para todas as consultas AWS."),
            |ui| {
                ui.horizontal_wrapped(|ui| {
                    field_combo(ui, "Perfil", &mut self.profile, 250.0);
                    if ui.button("Carregar perfis").clicked() {
                        self.load_profiles();
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    ui.label("Região");
                    ui.add(egui::TextEdit::singleline(&mut self.region).desired_width(130.0))
                        .on_hover_text("Região inicial: us-east-1. Confira a região antes de consultar ou atualizar o kubeconfig.");
                    ui.label("Cluster EKS");
                    ui.add(egui::TextEdit::singleline(&mut self.cluster).desired_width(220.0));
                });
                //ui.add_space(4.0);
                ui.horizontal_wrapped(|ui| {
                    if ui.button("AWS configure list").clicked() {
                        let args = self.aws_args(&["configure", "list"], false);
                        self.aws(Action::AwsConfigure, args);
                    }
                    if ui.button("Identidade STS").clicked() {
                        let args = self.aws_args(&["sts", "get-caller-identity"], false);
                        self.aws(Action::AwsIdentity, args);
                    }
                    if ui.button("Listar clusters EKS").clicked() {
                        let args = self.aws_args(&["eks", "list-clusters"], true);
                        self.aws(Action::EksClusters, args);
                    }
                    if ui.button("Atualizar kubeconfig").clicked() {
                        let cluster = self.cluster.trim().to_string();
                        if cluster.is_empty() {
                            self.status = "Informe o nome do cluster EKS.".into();
                        } else {
                            let args = self
                                .aws_args(&["eks", "update-kubeconfig", "--name", &cluster], true);
                            self.aws(Action::UpdateKubeconfig, args);
                        }
                    }
                });
            },
        );
    }

    fn show_aws_login(&mut self, ui: &mut egui::Ui) {
        let muted = palette_of(ui).muted;
        card(
            ui,
            "Autenticação (AWS SSO)",
            Some("O login abre o navegador e aguarda a autorização por até 5 minutos."),
            |ui| {
                ui.horizontal_wrapped(|ui| {
                    let enabled = !self.busy;
                    if primary_button(ui, enabled, "Login AWS SSO")
                        .on_hover_text("Executa `aws sso login` com o perfil selecionado.")
                        .clicked()
                    {
                        self.perform_login();
                    }
                });
                //ui.add_space(4.0);
                ui.label(
                    egui::RichText::new(
                        "Prefere autenticar fora do aplicativo? Execute em um terminal:",
                    )
                    .color(muted),
                );
                ui.horizontal(|ui| {
                    let sso = format!("aws sso login --profile {}", self.profile.trimmed());
                    ui.add(
                        egui::TextEdit::singleline(&mut sso.clone())
                            .desired_width(390.0)
                            .interactive(false),
                    );
                    if ui.button("Copiar comando SSO").clicked() {
                        ui.ctx().copy_text(sso);
                        self.status = "Comando SSO copiado.".into();
                    }
                });
                ui.label(
                    egui::RichText::new(
                        "Durante o login as demais ações ficam em espera; use Cancelar no painel de saída para interromper.",
                    )
                    .small()
                    .color(muted),
                );
            },
        );
    }
}
