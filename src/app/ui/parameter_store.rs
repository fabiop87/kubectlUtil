use super::*;

impl AppView {
    pub(super) fn parameters(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let mut list = card(
            "Parameter Store (AWS Systems Manager)",
            "Somente leitura. SecureString só é descriptografado quando você consulta o valor.",
        )
        .child(
            form_row()
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
                .child(self.field(
                    "parameter-filter",
                    "Filtrar nomes",
                    |m| &m.params.filter,
                    |m, v| m.params.filter = v,
                    window,
                    cx,
                )),
        )
        .child(
            row()
                .child(
                    self.button("load-params", "Carregar do cache", cx, |m| {
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
        );

        let needle = self.model.params.filter.to_lowercase();
        let busy = self.model.busy;
        let mut entries = div()
            .id("parameter-list")
            .max_h(px(260.))
            .overflow_y_scroll()
            .flex()
            .flex_col()
            .rounded(px(6.))
            .border_1()
            .border_color(rgb(LINE))
            .bg(rgb(WELL));
        for (i, entry) in self
            .model
            .params
            .entries
            .iter()
            .filter(|p| p.name.to_lowercase().contains(&needle))
            .enumerate()
        {
            let name = entry.name.clone();
            let selected = self.model.params.selected == entry.name;
            entries = entries.child(
                div()
                    .id(("parameter", i))
                    .flex()
                    .items_center()
                    .gap_4()
                    .px_3()
                    .py_2()
                    .border_l_2()
                    .border_color(if selected { rgb(ACCENT) } else { rgb(WELL) })
                    .bg(if selected { rgb(RAISED) } else { rgb(WELL) })
                    .hover(|style| style.bg(rgb(RAISED)))
                    .cursor_pointer()
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .font_family(MONO)
                            .child(entry.name.clone()),
                    )
                    .child(note(entry.kind.to_string()))
                    .child(note(entry.modified.to_string()))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !busy {
                            this.model.params.select(name.clone());
                            cx.notify();
                        }
                    })),
            );
        }
        list = list.child(entries).child(note(format!(
            "{} parâmetros em memória",
            self.model.params.entries.len()
        )));

        let (selected, has_selection) = if self.model.params.selected.is_empty() {
            ("Nenhum parâmetro selecionado".to_string(), false)
        } else {
            (self.model.params.selected.clone(), true)
        };
        let mut value = card(
            "Valor do parâmetro",
            "O valor é ocultado ao sair desta seção ou ao trocar de janela. Copiar envia o segredo para a área de transferência.",
        )
        .child(
            div()
                .font_family(MONO)
                .text_color(if has_selection { rgb(TEXT) } else { rgb(MUTED) })
                .child(selected),
        )
        .child(
            self.button("reveal", "Consultar valor", cx, |m| {
                m.reveal_selected_parameter()
            })
            .disabled(self.model.busy || !has_selection)
            .primary(),
        );
        if self.model.params.revealed_visible {
            // Âmbar na borda: há um segredo na tela.
            value = value
                .border_color(rgb(WARN))
                .child(
                    row()
                        .child(
                            Button::new("hide-value")
                                .label("Ocultar valor")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.model.params.hide_value();
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("copy-value")
                                .label("Copiar valor")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        this.model.params.revealed.clone(),
                                    ));
                                    this.model.status = "Valor copiado para a área de transferência; limpe-a se necessário.".into();
                                    cx.notify();
                                })),
                        ),
                )
                .child(
                    theme::well()
                        .id("revealed-value")
                        .max_h(px(160.))
                        .overflow_scroll()
                        .child(self.model.params.revealed.clone()),
                );
        } else {
            value = value.child(note(
                "Valor oculto. A consulta individual depende de uma ação explícita.",
            ));
        }
        blocks([list, value])
    }
}
