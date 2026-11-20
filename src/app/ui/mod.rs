//! Janela GPUI Kit; o modelo mantém a execução e a validação dos comandos.
use super::{KubectlUtilApp, MockScenario, Section};
use crate::theme::{
    self, ACCENT, BG, DANGER, LINE, MONO, MUTED, RAISED, SURFACE, TEXT, WARN, WELL,
};
use gpui_kit::{
    component::{
        Disableable, Sizable,
        button::{Button, ButtonVariants},
        input::{InputState, TextareaState},
        select::{SearchableVec, SelectState},
    },
    *,
};
use std::{collections::HashMap, sync::atomic::Ordering, time::Duration};

mod aws;
mod connection;
mod parameter_store;
mod sections;
mod widgets;
use gpui_kit::component::Selectable as KitSelectable;
use widgets::{blocks, card, form_row, note, row};

type Choices = SelectState<SearchableVec<SharedString>>;
pub struct AppView {
    model: KubectlUtilApp,
    inputs: HashMap<&'static str, Entity<InputState>>,
    selects: HashMap<&'static str, (Entity<Choices>, Vec<String>)>,
    subscriptions: Vec<Subscription>,
    console: Entity<TextareaState>,
    history_open: bool,
}

impl AppView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let console = cx.new(|cx| TextareaState::new(window, cx));
        cx.spawn_in(window, async move |view, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                if view
                    .update_in(cx, |view, window, cx| {
                        let mut changed = false;
                        while let Ok(result) = view.model.rx.try_recv() {
                            view.model.handle_result(result);
                            changed = true;
                        }
                        if view.model.port_forwards.has_processes() {
                            let count = view.model.port_forwards.len();
                            view.model.poll_port_forwards();
                            changed |= count != view.model.port_forwards.len();
                        }
                        if !window.is_window_active() && view.model.params.revealed_visible {
                            view.model.params.hide_value();
                            changed = true;
                        }
                        if changed {
                            cx.notify();
                        }
                    })
                    .is_err()
                {
                    break;
                }
            }
        })
        .detach();
        Self {
            model: KubectlUtilApp::new(),
            inputs: HashMap::new(),
            selects: HashMap::new(),
            subscriptions: Vec::new(),
            console,
            history_open: false,
        }
    }

    fn button(
        &self,
        id: &'static str,
        label: &'static str,
        cx: &Context<Self>,
        action: impl Fn(&mut KubectlUtilApp) + 'static,
    ) -> Button {
        Button::new(id)
            .label(label)
            .disabled(self.model.busy)
            .on_click(cx.listener(move |this, _, _, cx| {
                action(&mut this.model);
                cx.notify();
            }))
    }

    fn navigation(&self, cx: &Context<Self>) -> Div {
        let mut nav = div()
            .w(px(188.))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_1()
            .px_3()
            .py_4()
            .bg(rgb(SURFACE))
            .border_r_1()
            .border_color(rgb(LINE))
            .child(
                div()
                    .px_2()
                    .pb_4()
                    .text_lg()
                    .font_weight(FontWeight::BOLD)
                    .child("kubectlUtil"),
            );
        for (index, section) in [
            Section::Connection,
            Section::Overview,
            Section::Cluster,
            Section::Network,
            Section::Aws,
            Section::Parameters,
        ]
        .into_iter()
        .enumerate()
        {
            let selected = self.model.section == section;
            nav = nav.child(
                div()
                    .border_l_2()
                    .border_color(if selected { rgb(ACCENT) } else { rgb(SURFACE) })
                    .child(
                        Button::new(("nav", index))
                            .label(section.label())
                            .w_full()
                            .h(px(36.))
                            .selected(selected)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.model.navigate(section);
                                cx.notify();
                            })),
                    ),
            );
        }
        nav.child(div().flex_1()).child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .pt_3()
                .border_t_1()
                .border_color(rgb(LINE))
                .child(
                    div()
                        .text_xs()
                        .text_color(if self.model.mock_mode { rgb(WARN) } else { rgb(MUTED) })
                        .child(if self.model.mock_mode {
                            "Ambiente simulado"
                        } else {
                            "Comandos reais"
                        }),
                )
                .child(self.button(
                    "mock",
                    if self.model.mock_mode {
                        "Desativar Mock"
                    } else {
                        "Ativar modo Mock"
                    },
                    cx,
                    |m| {
                        m.mock_mode = !m.mock_mode;
                        m.on_mock_toggled();
                    },
                ))
                .children(self.model.mock_mode.then(|| {
                    row().children(MockScenario::ALL.into_iter().enumerate().map(
                        |(i, scenario)| {
                            Button::new(("scenario", i))
                                .label(scenario.label())
                                .small()
                                .disabled(self.model.busy)
                                .selected(self.model.mock_scenario == scenario)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.model.mock_scenario = scenario;
                                    cx.notify();
                                }))
                        },
                    ))
                })),
        )
    }

    /// Barra de alvo: onde os comandos vão agir. A faixa lateral muda de cor
    /// em modo mock (âmbar) e quando o contexto parece de produção (vermelho).
    fn target_bar(&self) -> Div {
        let m = &self.model;
        let prod = !m.mock_mode && is_prod(&m.context.value);
        let tone = if m.mock_mode {
            WARN
        } else if prod {
            DANGER
        } else {
            ACCENT
        };
        let tag = if m.mock_mode {
            Some("Simulado: nenhum comando real é executado")
        } else if prod {
            Some("Produção")
        } else {
            None
        };
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_6()
            .px_4()
            .py_3()
            .rounded(px(8.))
            .border_1()
            .border_color(rgb(LINE))
            .bg(rgb(SURFACE))
            .child(div().w(px(4.)).h(px(34.)).rounded_full().bg(rgb(tone)))
            .child(widgets::target("Perfil AWS", &m.profile.value, TEXT))
            .child(widgets::target("Região", &m.region, TEXT))
            .child(widgets::target(
                "Contexto",
                &m.context.value,
                if prod { DANGER } else { TEXT },
            ))
            .child(widgets::target("Namespace", &m.namespace.value, TEXT))
            .child(div().flex_1())
            .children(tag.map(|text| {
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(tone))
                    .child(text)
            }))
    }

    fn output(&mut self, window: &mut Window, cx: &mut Context<Self>) -> Div {
        let value = self.model.output.clone();
        if self.console.read(cx).value().as_ref() != value {
            self.console
                .update(cx, |state, cx| state.set_value(value, window, cx));
        }
        let mut actions = row()
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child(self.model.output_title.clone()),
            )
            .children(self.model.busy.then(|| {
                div().text_xs().text_color(rgb(WARN)).child("Executando")
            }))
            .child(div().flex_1())
            .child(
                Button::new("copy-output")
                    .label("Copiar")
                    .small()
                    .on_click(cx.listener(|this, _, _, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(this.model.output.clone()));
                        this.model.status = "Saída copiada.".into();
                        cx.notify();
                    })),
            )
            .child(
                Button::new("save-output")
                    .label("Salvar .log")
                    .small()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.model.save_output_log();
                        cx.notify();
                    })),
            )
            .child(
                Button::new("clear-output")
                    .label("Limpar")
                    .small()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.model.output.clear();
                        cx.notify();
                    })),
            );
        if self.model.busy {
            actions = actions.child(
                Button::new("cancel")
                    .label("Cancelar")
                    .danger()
                    .small()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.model.cancel.store(true, Ordering::Relaxed);
                        this.model.status = "Cancelando…".into();
                        cx.notify();
                    })),
            );
        }
        theme::panel().flex_shrink_0().gap_2().child(actions).child(
            theme::well().p_2().child(
                gpui_kit::component::input::Textarea::new(&self.console)
                    .readonly(true)
                    .appearance(false)
                    .h(px(190.))
                    .aria_label("Saída dos comandos"),
            ),
        )
    }

    fn history(&self, cx: &Context<Self>) -> Div {
        let mut result = card("Histórico local", "Últimas operações desta sessão.").child(
            Button::new("toggle-history")
                .label(format!(
                    "{} entradas · {}",
                    self.model.history.len(),
                    if self.history_open {
                        "Recolher"
                    } else {
                        "Expandir"
                    }
                ))
                .on_click(cx.listener(|this, _, _, cx| {
                    this.history_open = !this.history_open;
                    cx.notify();
                })),
        );
        if self.history_open {
            result = result
                .child(
                    row()
                        .child(
                            Button::new("copy-history")
                                .label("Copiar histórico")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        this.model.history.join("\n"),
                                    ));
                                })),
                        )
                        .child(
                            Button::new("clear-history")
                                .label("Limpar histórico")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.model.history.clear();
                                    cx.notify();
                                })),
                        ),
                )
                .child(
                    theme::well()
                        .id("history-lines")
                        .max_h(px(160.))
                        .overflow_y_scroll()
                        .children(
                            self.model
                                .history
                                .iter()
                                .rev()
                                .take(50)
                                .map(|line| div().child(line.clone())),
                        ),
                );
        }
        result
    }
}

impl Render for AppView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !window.is_window_active() {
            self.model.params.hide_value();
        }
        let content = match self.model.section {
            Section::Connection => self.connection(window, cx),
            Section::Overview => self.presets("Acesso rápido", super::presets::OVERVIEW, cx),
            Section::Cluster => self.presets("Consultas do cluster", super::presets::CLUSTER, cx),
            Section::Network => {
                self.presets("Rede, storage e backups", super::presets::NETWORK, cx)
            }
            Section::Aws => self.aws(window, cx),
            Section::Parameters => self.parameters(window, cx),
        };
        div()
            .size_full()
            .flex()
            .text_color(rgb(TEXT))
            .text_sm()
            .bg(rgb(BG))
            .child(self.navigation(cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .p_4()
                    .child(self.target_bar())
                    .child(
                        div()
                            .id("workspace-scroll")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_4()
                                    .pb_2()
                                    .child(content)
                                    .child(self.history(cx)),
                            ),
                    )
                    .child(self.output(window, cx))
                    .child(
                        row()
                            .gap_2()
                            .child(div().w(px(8.)).h(px(8.)).rounded_full().bg(
                                if self.model.busy { rgb(WARN) } else { rgb(LINE) },
                            ))
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(MUTED))
                                    .child(self.model.status.clone()),
                            ),
                    ),
            )
    }
}

/// Heurística: contextos com "prod" ou "prd" no nome destacam o alvo em vermelho.
fn is_prod(context: &str) -> bool {
    let name = context.to_lowercase();
    ["prod", "prd"].iter().any(|key| name.contains(key))
}