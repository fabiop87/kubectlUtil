//! Componentes compartilhados com estados de entrada persistentes do Kit.
use super::*;
use gpui_kit::component::{
    input::{Input, InputEvent},
    select::{Select, SelectEvent},
};

const FIELD_W: f32 = 280.;

/// Linha de botões ou itens; centraliza na vertical.
pub(super) fn row() -> Div {
    div().flex().flex_wrap().items_center().gap_2()
}
/// Linha com campos e botões: os botões alinham com a base do campo, não com o rótulo.
pub(super) fn form_row() -> Div {
    div().flex().flex_wrap().items_end().gap_2()
}
pub(super) fn blocks(cards: impl IntoIterator<Item = Div>) -> Div {
    div().flex().flex_wrap().items_start().gap_4().children(
        cards
            .into_iter()
            .map(|card| card.flex_1().flex_basis(px(420.)).min_w_0()),
    )
}
pub(super) fn note(text: impl Into<SharedString>) -> Div {
    div().text_xs().text_color(rgb(MUTED)).child(text.into())
}
pub(super) fn card(title: &'static str, subtitle: &'static str) -> Div {
    theme::panel()
        .child(
            div()
                .text_base()
                .font_weight(FontWeight::SEMIBOLD)
                .child(title),
        )
        .child(note(subtitle))
}
/// Item da barra de alvo: rótulo pequeno e valor em monoespaçada.
pub(super) fn target(label: &'static str, value: &str, color: u32) -> Div {
    let empty = value.is_empty();
    div()
        .flex()
        .flex_col()
        .gap_1()
        .min_w_0()
        .child(note(label))
        .child(
            div()
                .font_family(MONO)
                .text_color(if empty { rgb(MUTED) } else { rgb(color) })
                .child(if empty {
                    "não definido".to_string()
                } else {
                    value.to_string()
                }),
        )
}

impl AppView {
    pub(super) fn field(
        &mut self,
        id: &'static str,
        label: &'static str,
        get: fn(&KubectlUtilApp) -> &String,
        set: fn(&mut KubectlUtilApp, String),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let value = get(&self.model).clone();
        let state = if let Some(state) = self.inputs.get(id) {
            state.clone()
        } else {
            let state = cx.new(|cx| {
                InputState::new(window, cx)
                    .default_value(value.clone())
                    .placeholder(label)
            });
            self.subscriptions.push(cx.subscribe_in(
                &state,
                window,
                move |this, state, event, _, cx| {
                    if matches!(event, InputEvent::Change) {
                        set(&mut this.model, state.read(cx).value().to_string());
                        cx.notify();
                    }
                },
            ));
            self.inputs.insert(id, state.clone());
            state
        };
        if state.read(cx).value().as_ref() != value {
            state.update(cx, |state, cx| state.set_value(value, window, cx));
        }
        div()
            .w(px(FIELD_W))
            .max_w_full()
            .flex()
            .flex_col()
            .gap_1()
            .child(note(label))
            .child(Input::new(&state).aria_label(label))
    }

    pub(super) fn choice(
        &mut self,
        id: &'static str,
        label: &'static str,
        get: fn(&KubectlUtilApp) -> &super::super::Selectable,
        set: fn(&mut KubectlUtilApp, String),
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let selectable = get(&self.model);
        let value = selectable.value.clone();
        let mut items = selectable.items.clone();
        if !value.is_empty() && !items.contains(&value) {
            items.push(value.clone());
        }
        let data = || {
            SearchableVec::new(
                items
                    .iter()
                    .cloned()
                    .map(SharedString::from)
                    .collect::<Vec<_>>(),
            )
        };
        let state = if let Some((state, previous)) = self.selects.get_mut(id) {
            if *previous != items {
                state.update(cx, |state, cx| state.set_items(data(), window, cx));
                *previous = items.clone();
            }
            state.clone()
        } else {
            let state = cx.new(|cx| SelectState::new(data(), None, window, cx).searchable(true));
            self.subscriptions.push(cx.subscribe_in(
                &state,
                window,
                move |this, _, event, _, cx| {
                    let SelectEvent::Confirm(value) = event;
                    set(
                        &mut this.model,
                        value.as_ref().map(ToString::to_string).unwrap_or_default(),
                    );
                    cx.notify();
                },
            ));
            self.selects.insert(id, (state.clone(), items.clone()));
            state
        };
        if state
            .read(cx)
            .selected_value()
            .map(|s| s.as_ref())
            .unwrap_or("")
            != value
        {
            state.update(cx, |state, cx| {
                state.set_selected_value(&SharedString::from(value), window, cx)
            });
        }
        div()
            .w(px(FIELD_W))
            .max_w_full()
            .flex()
            .flex_col()
            .gap_1()
            .child(note(label))
            .child(
                Select::new(&state)
                    .placeholder("Selecionar…")
                    .search_placeholder("Pesquisar…")
                    .disabled(self.model.busy)
                    .w_full(),
            )
    }
}