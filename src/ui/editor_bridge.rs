use crate::autocomplete::engine::AutocompleteEngine;
use crate::syntax::intel8080::highlight_document;
use crate::ui::controller::IdeController;
use crate::ui::{AppWindow, DocItem, HighlightLineItem, LabelItem, SuggestionItem, TokenItem};
use slint::{ModelRc, SharedString, VecModel};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

pub fn update_highlight_and_labels(
    app: &AppWindow,
    autocomplete: &AutocompleteEngine,
    code: &str,
    is_asm: bool,
) {
    let line_count = code.lines().count().max(1);
    let mut numbers_str = String::with_capacity(line_count * 5);
    for i in 1..=line_count {
        numbers_str.push_str(&i.to_string());
        numbers_str.push('\n');
    }
    app.set_line_numbers_text(SharedString::from(numbers_str));

    if !is_asm {
        app.set_highlight_lines(ModelRc::default());
        app.set_label_list(ModelRc::default());
        return;
    }

    let lines = highlight_document(code);
    let line_models: Vec<HighlightLineItem> = lines
        .into_iter()
        .map(|line| {
            let token_models: Vec<TokenItem> = line
                .tokens
                .into_iter()
                .map(|tok| TokenItem {
                    text: SharedString::from(tok.text),
                    color: tok.color,
                })
                .collect();
            HighlightLineItem {
                line_number: line.line_number,
                tokens: ModelRc::from(Rc::new(VecModel::from(token_models))),
            }
        })
        .collect();
    app.set_highlight_lines(ModelRc::from(Rc::new(VecModel::from(line_models))));

    let labels = autocomplete.get_user_labels(code);
    let label_items: Vec<LabelItem> = labels
        .into_iter()
        .map(|l| LabelItem {
            name: SharedString::from(l.name),
            line_number: l.line_number as i32,
        })
        .collect();
    app.set_label_list(ModelRc::from(Rc::new(VecModel::from(label_items))));
}

pub fn populate_docs_ui(app: &AppWindow, lang: &str) {
    let doc_items: Vec<DocItem> = crate::autocomplete::intel8080_docs::INTEL_8080_DOCS
        .iter()
        .map(|doc| DocItem {
            name: SharedString::from(doc.name),
            syntax: SharedString::from(doc.syntax),
            summary: SharedString::from(doc.get_summary(lang)),
            operands: SharedString::from(doc.operands),
            flags: SharedString::from(doc.flags),
            cycles: SharedString::from(doc.cycles),
            bytes: doc.bytes as i32,
            category: SharedString::from(doc.get_category(lang)),
        })
        .collect();
    app.set_docs_list(ModelRc::from(Rc::new(VecModel::from(doc_items))));
}

pub fn refresh_suggestions_ui(
    app: &AppWindow,
    controller_rc: &Arc<Mutex<IdeController>>,
    query: &str,
    lang: &str,
) {
    let mut controller = controller_rc.lock().unwrap();
    let current_code = app.get_code_text().to_string();
    let suggestions = controller
        .autocomplete
        .get_suggestions_lang(query, &current_code, lang);
    controller.active_suggestions = suggestions.clone();

    if let Some(first) = suggestions.first() {
        app.set_selected_sugg_name(SharedString::from(&first.name));
        app.set_selected_sugg_category(SharedString::from(&first.category));
        app.set_selected_sugg_syntax(SharedString::from(&first.syntax));
        app.set_selected_sugg_summary(SharedString::from(&first.summary));
        app.set_selected_sugg_flags(SharedString::from(&first.flags));
        app.set_selected_sugg_cycles(SharedString::from(&first.cycles));
        app.set_selected_sugg_insert_text(SharedString::from(&first.insert_text));
        app.set_selected_sugg_icon_kind(SharedString::from(&first.icon_kind));
        app.set_selected_sugg_example(SharedString::from(&first.example));
    }

    let items: Vec<SuggestionItem> = suggestions
        .into_iter()
        .map(|s| SuggestionItem {
            name: SharedString::from(s.name),
            category: SharedString::from(s.category),
            syntax: SharedString::from(s.syntax),
            summary: SharedString::from(s.summary),
            flags: SharedString::from(s.flags),
            cycles: SharedString::from(s.cycles),
            insert_text: SharedString::from(s.insert_text),
            icon_kind: SharedString::from(s.icon_kind),
            example: SharedString::from(s.example),
        })
        .collect();
    app.set_suggestion_list(ModelRc::from(Rc::new(VecModel::from(items))));
}
