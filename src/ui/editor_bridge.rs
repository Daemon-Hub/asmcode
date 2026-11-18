use crate::autocomplete::engine::AutocompleteEngine;
use crate::syntax::intel8080::highlight_document;
use crate::ui::controller::IdeController;
use crate::ui::{AppWindow, DocItem, HighlightLineItem, LabelItem, SuggestionItem, TokenItem};
use slint::{ModelRc, SharedString, VecModel};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

#[inline]
fn push_usize(buf: &mut String, mut n: usize) {
    if n == 0 {
        buf.push('0');
        return;
    }
    let mut digits = [0u8; 20];
    let mut pos = 20;
    while n > 0 {
        pos -= 1;
        digits[pos] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    // SAFETY: digits[pos..20] contains ASCII decimal digits '0'..'9', guaranteed valid UTF-8
    let s = unsafe { std::str::from_utf8_unchecked(&digits[pos..]) };
    buf.push_str(s);
}

pub fn update_highlight_and_labels(
    app: &AppWindow,
    autocomplete: &AutocompleteEngine,
    code: &str,
    is_asm: bool,
) {
    let line_count = if code.is_empty() {
        1
    } else {
        code.as_bytes().iter().filter(|&&b| b == b'\n').count() + 1
    };

    let is_large_file = app.get_is_large_file_mode();

    // Fast O(1) bounded numbers generation
    let max_numbers = if is_large_file {
        100_000
    } else {
        line_count.min(300_000)
    };
    let actual_count = line_count.min(max_numbers);
    let mut numbers_str = String::with_capacity(actual_count * 6);
    for i in 1..=actual_count {
        push_usize(&mut numbers_str, i);
        numbers_str.push('\n');
    }
    app.set_line_numbers_text(SharedString::from(numbers_str));

    if !is_asm || is_large_file {
        app.set_highlight_lines(ModelRc::default());
        app.set_label_list(ModelRc::default());
        return;
    }

    let theme_id = app.get_active_theme_id();
    let lines = highlight_document(code, theme_id);
    let line_models: Vec<HighlightLineItem> = lines
        .into_iter()
        .map(|line| {
            let token_models: Vec<TokenItem> = line
                .tokens
                .into_iter()
                .map(|tok| TokenItem {
                    text: tok.text,
                    color: tok.color,
                    is_italic: tok.is_italic,
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

pub fn populate_filtered_docs_ui(app: &AppWindow, lang: &str, category: &str) {
    let docs = crate::autocomplete::intel8080_docs::INTEL_8080_DOCS;
    let theme_id = app.get_active_theme_id();
    let doc_items: Vec<DocItem> = docs
        .iter()
        .filter(|doc| {
            if category.is_empty() || category == "Все" || category == "All" {
                return true;
            }
            let cat_ru = doc.get_category("ru");
            let cat_en = doc.get_category("en");
            let cat_lower = category.to_lowercase();
            cat_ru.to_lowercase().contains(&cat_lower)
                || cat_en.to_lowercase().contains(&cat_lower)
                || doc.category.to_lowercase().contains(&cat_lower)
                || doc.category_ru.to_lowercase().contains(&cat_lower)
        })
        .map(|doc| {
            let detailed_desc =
                crate::autocomplete::intel8080_details::get_detailed_description(doc.name, lang);
            let when_to_use =
                crate::autocomplete::intel8080_details::get_when_to_use(doc.name, lang);
            let notes = crate::autocomplete::intel8080_details::get_notes(doc.name, lang);
            let code_example =
                crate::autocomplete::intel8080_details::get_detailed_code_example(doc.name);

            // True syntax highlighting using active theme!
            let highlighted = crate::syntax::intel8080::highlight_document(code_example, theme_id);
            let example_lines: Vec<HighlightLineItem> = highlighted
                .into_iter()
                .map(|hl| {
                    let tokens: Vec<TokenItem> = hl
                        .tokens
                        .into_iter()
                        .map(|t| TokenItem {
                            text: t.text,
                            color: t.color,
                            is_italic: t.is_italic,
                        })
                        .collect();
                    HighlightLineItem {
                        line_number: hl.line_number,
                        tokens: ModelRc::from(Rc::new(VecModel::from(tokens))),
                    }
                })
                .collect();

            DocItem {
                name: SharedString::from(doc.name),
                syntax: SharedString::from(doc.syntax),
                summary: SharedString::from(doc.get_summary(lang)),
                operands: SharedString::from(doc.operands),
                flags: SharedString::from(doc.flags),
                cycles: SharedString::from(doc.cycles),
                bytes: doc.bytes as i32,
                category: SharedString::from(doc.get_category(lang)),
                description: SharedString::from(detailed_desc),
                when_to_use: SharedString::from(when_to_use),
                notes: SharedString::from(notes),
                raw_snippet: SharedString::from(code_example),
                example_lines: ModelRc::from(Rc::new(VecModel::from(example_lines))),
                icon_kind: SharedString::from(doc.get_icon_kind()),
            }
        })
        .collect();
    let total_count = docs.len() as i32;
    app.set_docs_list(ModelRc::from(Rc::new(VecModel::from(doc_items))));
    app.set_docs_total_count(total_count);
}

pub fn populate_docs_ui(app: &AppWindow, lang: &str) {
    populate_filtered_docs_ui(app, lang, "Все");
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
        app.set_selected_sugg_name(first.name.clone());
        app.set_selected_sugg_category(first.category.clone());
        app.set_selected_sugg_syntax(first.syntax.clone());
        app.set_selected_sugg_summary(first.summary.clone());
        app.set_selected_sugg_flags(first.flags.clone());
        app.set_selected_sugg_cycles(first.cycles.clone());
        app.set_selected_sugg_insert_text(first.insert_text.clone());
        app.set_selected_sugg_icon_kind(first.icon_kind.clone());
        app.set_selected_sugg_example(first.example.clone());
    }

    let items: Vec<SuggestionItem> = suggestions
        .into_iter()
        .map(|s| SuggestionItem {
            name: s.name,
            category: s.category,
            syntax: s.syntax,
            summary: s.summary,
            flags: s.flags,
            cycles: s.cycles,
            insert_text: s.insert_text,
            icon_kind: s.icon_kind,
            example: s.example,
        })
        .collect();
    app.set_suggestion_list(ModelRc::from(Rc::new(VecModel::from(items))));
}

pub fn ensure_active_file_or_create_draft(app: &AppWindow, ctrl: &mut IdeController) {
    if ctrl.file_manager.active_file_path.is_some() && !app.get_active_file_name().is_empty() {
        return;
    }

    let mut idx = 1;
    let draft_name = loop {
        let name = if idx == 1 {
            "NewFile.asm".to_string()
        } else {
            format!("NewFile_{}.asm", idx)
        };
        let path = ctrl.file_manager.current_dir.join(&name);
        if !path.exists() && !ctrl.open_tabs.contains(&path) {
            break name;
        }
        idx += 1;
    };

    let draft_path = ctrl.file_manager.current_dir.join(&draft_name);
    ctrl.untitled_drafts.insert(draft_path.clone());
    if !ctrl.open_tabs.contains(&draft_path) {
        ctrl.open_tabs.push(draft_path.clone());
    }
    ctrl.file_manager.active_file_path = Some(draft_path.clone());

    app.set_active_file_name(SharedString::from(&draft_name));
    app.set_active_file_path(SharedString::from(draft_path.to_string_lossy().to_string()));
    app.set_is_dirty(true);
    app.set_is_active_asm(true);
}

pub fn record_editor_mutation(
    app: &AppWindow,
    ctrl: &mut IdeController,
    new_code: &str,
    cursor_pos: i32,
    is_group: bool,
) {
    ensure_active_file_or_create_draft(app, ctrl);
    ctrl.undo_manager
        .record_edit(new_code, cursor_pos, is_group);
    if let Some(path) = &ctrl.file_manager.active_file_path {
        match ctrl.tab_buffers.get_mut(path) {
            Some(buf) => {
                buf.clear();
                buf.push_str(new_code);
            }
            None => {
                ctrl.tab_buffers.insert(path.clone(), new_code.to_string());
            }
        }
    }
}
