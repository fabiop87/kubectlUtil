use super::*;

impl AppView {
    pub(super) fn parameters(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let mut list = card(
            "AWS Systems Manager · Parameter Store",
            "Somente leitura. SecureString só é descriptografado quando você consulta o valor.",
        )
        .child(note(format!(
            "Perfil: {} · Região: {}",
            self.model.profile.value, self.model.region
        )))
        .child(self.field(
            "parameter-path",
            "Caminho",
            |m| &m.params.path,
            |m, v| {
                m.params.path = v;
                m.params.hide_value();
            },
            window,
            cx,
        ))
        .child(
            row()
                .child(
                    self.button("load-params", "Carregar (usar cache)", cx, |m| {
                        m.load_parameters(false)
                    })
                    .primary(),
                )
                .child(self.button("refresh-params", "Atualizar da AWS", cx, |m| {
                    m.load_parameters(true)
                }))
                .child(self.button("clear-cache", "Limpar cache", cx, |m| {
                    m.clear_parameter_cache()
                })),
        )
        .child(self.field(
            "parameter-filter",
            "Filtrar nomes",
            |m| &m.params.filter,
            |m, v| m.params.filter = v,
            window,
            cx,
        ))
        .child(note(format!(
            "{} parâmetros em memória",
            self.model.params.entries.len()
        )));
        let needle = self.model.params.filter.to_lowercase();
        let mut entries = div()
            .id("parameter-list")
            .max_h(px(220.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .gap_2();
        for (i, entry) in self
            .model
            .params
            .entries
            .iter()
            .filter(|p| p.name.to_lowercase().contains(&needle))
            .enumerate()
        {
            let name = entry.name.clone();
            entries = entries.child(
                row()
                    .child(
                        Button::new(("parameter", i))
                            .label(entry.name.clone())
                            .selected(self.model.params.selected == entry.name)
                            .disabled(self.model.busy)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.model.params.select(name.clone());
                                cx.notify();
                            })),
                    )
                    .child(note(format!("{} · {}", entry.kind, entry.modified))),
            );
        }
        list = list.child(entries);
        let selected = if self.model.params.selected.is_empty() {
            "Nenhum parâmetro selecionado".to_string()
        } else {
            self.model.params.selected.clone()
        };
        let mut value=card("Valor do parâmetro", "Ao sair desta seção ou desativar a janela, o valor é ocultado. Copiar envia o segredo à área de transferência do sistema.")
            .child(div().child(selected))
            .child(self.button("reveal", "Consultar valor", cx, |m| m.reveal_selected_parameter())
                .disabled(self.model.busy || self.model.params.selected.is_empty()).primary());
        if self.model.params.revealed_visible {
            value=value.child(row()
                .child(Button::new("hide-value").label("Ocultar valor").on_click(cx.listener(|this, _, _, cx| {
                    this.model.params.hide_value(); cx.notify();
                })))
                .child(Button::new("copy-value").label("Copiar valor").on_click(cx.listener(|this, _, _, cx| {
                    cx.write_to_clipboard(ClipboardItem::new_string(this.model.params.revealed.clone()));
                    this.model.status="Valor copiado para a área de transferência; limpe-a se necessário.".into(); cx.notify();
                }))))
                .child(div().id("revealed-value").max_h(px(160.)).overflow_scroll().p_3().rounded(px(10.))
                    .bg(rgb(0x130C1F)).font_family("Consolas").child(self.model.params.revealed.clone()));
        } else {
            value = value.child(note(
                "Valor oculto. A consulta individual depende de uma ação explícita.",
            ));
        }
        blocks([list, value])
    }
}
