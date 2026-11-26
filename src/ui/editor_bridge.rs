use crate::autocomplete::engine::AutocompleteEngine;
use crate::syntax::intel8080::highlight_document_range_with_offsets;
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

pub fn update_gutter_line_numbers(
    app: &AppWindow,
    total_lines: usize,
    scroll_line: usize,
    visible_count: usize,
) -> (usize, usize) {
    let total = total_lines.max(1);
    let pad = 35;
    let start_line = scroll_line.saturating_sub(pad).min(total.saturating_sub(1));
    let count = (visible_count + pad * 2).clamp(60, 150);
    let end_line = (start_line + count).min(total);

    let mut numbers_str = String::with_capacity((end_line - start_line) * 7);
    for i in (start_line + 1)..=end_line {
        push_usize(&mut numbers_str, i);
        numbers_str.push('\n');
    }
    app.set_gutter_start_line(start_line as i32);
    app.set_line_numbers_text(SharedString::from(numbers_str));
    (start_line, end_line)
}

pub fn update_viewport_highlight(
    app: &AppWindow,
    code: &str,
    line_starts: Option<&[usize]>,
    scroll_line: usize,
    visible_count: usize,
    is_asm: bool,
) -> (usize, usize) {
    let total_lines = if code.is_empty() {
        1
    } else if let Some(starts) = line_starts {
        starts.len().max(1)
    } else {
        code.as_bytes().iter().filter(|&&b| b == b'\n').count() + 1
    };

    let gutter_range = update_gutter_line_numbers(app, total_lines, scroll_line, visible_count);

    if !is_asm || code.is_empty() || app.get_is_large_file_mode() {
        app.set_highlight_lines(ModelRc::default());
        return gutter_range;
    }

    let theme_id = app.get_active_theme_id();
    let pad = 35;
    let buffered_start = scroll_line.saturating_sub(pad);
    let count = (visible_count + pad * 2).clamp(60, 150);

    let lines = highlight_document_range_with_offsets(code, line_starts, theme_id, buffered_start, count);
    let rendered_count = lines.len();
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
                    char_count: tok.char_count,
                    is_whitespace: tok.is_whitespace,
                })
                .collect();
            HighlightLineItem {
                line_number: line.line_number,
                tokens: ModelRc::from(Rc::new(VecModel::from(token_models))),
            }
        })
        .collect();

    app.set_highlight_start_line(buffered_start as i32);
    app.set_highlight_lines(ModelRc::from(Rc::new(VecModel::from(line_models))));

    (buffered_start, buffered_start + rendered_count)
}

pub fn sync_labels_to_ui(
    app: &AppWindow,
    labels: &[crate::autocomplete::extractor::UserSymbol],
) {
    let label_items: Vec<LabelItem> = labels
        .iter()
        .take(150)
        .map(|l| LabelItem {
            name: SharedString::from(l.name.as_str()),
            line_number: l.line_number as i32,
        })
        .collect();
    app.set_label_list(ModelRc::from(Rc::new(VecModel::from(label_items))));
}

pub fn update_highlight_and_labels_opts(
    app: &AppWindow,
    autocomplete: &AutocompleteEngine,
    code: &str,
    line_starts: Option<&[usize]>,
    is_asm: bool,
    refresh_line_numbers: bool,
) -> Vec<crate::autocomplete::extractor::UserSymbol> {
    let scroll_line = app.get_visible_scroll_line().max(0) as usize;
    let visible_count = (app.get_visible_line_count().max(30) as usize).min(100);

    if !is_asm || app.get_is_large_file_mode() {
        if refresh_line_numbers {
            let total_lines = if code.is_empty() {
                1
            } else if let Some(starts) = line_starts {
                starts.len().max(1)
            } else {
                code.as_bytes().iter().filter(|&&b| b == b'\n').count() + 1
            };
            update_gutter_line_numbers(app, total_lines, scroll_line, visible_count);
        }
        app.set_highlight_lines(ModelRc::default());
        app.set_label_list(ModelRc::default());
        return Vec::new();
    }

    if refresh_line_numbers {
        update_viewport_highlight(app, code, line_starts, scroll_line, visible_count, is_asm);
    } else {
        let theme_id = app.get_active_theme_id();
        let pad = 35;
        let buffered_start = scroll_line.saturating_sub(pad);
        let count = (visible_count + pad * 2).clamp(60, 150);
        let lines =
            highlight_document_range_with_offsets(code, line_starts, theme_id, buffered_start, count);
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
                        char_count: tok.char_count,
                        is_whitespace: tok.is_whitespace,
                    })
                    .collect();
                HighlightLineItem {
                    line_number: line.line_number,
                    tokens: ModelRc::from(Rc::new(VecModel::from(token_models))),
                }
            })
            .collect();
        app.set_highlight_start_line(buffered_start as i32);
        app.set_highlight_lines(ModelRc::from(Rc::new(VecModel::from(line_models))));
    }

    let labels = autocomplete.get_user_labels_with_starts(code, line_starts);
    sync_labels_to_ui(app, &labels);
    labels
}

pub fn update_highlight_only_opts(
    app: &AppWindow,
    code: &str,
    line_starts: Option<&[usize]>,
    is_asm: bool,
    refresh_line_numbers: bool,
) {
    let scroll_line = app.get_visible_scroll_line().max(0) as usize;
    let visible_count = (app.get_visible_line_count().max(30) as usize).min(100);
    if refresh_line_numbers {
        update_viewport_highlight(app, code, line_starts, scroll_line, visible_count, is_asm);
    } else if is_asm && !app.get_is_large_file_mode() {
        let theme_id = app.get_active_theme_id();
        let pad = 35;
        let buffered_start = scroll_line.saturating_sub(pad);
        let count = (visible_count + pad * 2).clamp(60, 150);
        let lines =
            highlight_document_range_with_offsets(code, line_starts, theme_id, buffered_start, count);
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
                        char_count: tok.char_count,
                        is_whitespace: tok.is_whitespace,
                    })
                    .collect();
                HighlightLineItem {
                    line_number: line.line_number,
                    tokens: ModelRc::from(Rc::new(VecModel::from(token_models))),
                }
            })
            .collect();
        app.set_highlight_start_line(buffered_start as i32);
        app.set_highlight_lines(ModelRc::from(Rc::new(VecModel::from(line_models))));
    }
}

pub fn update_highlight_and_labels(
    app: &AppWindow,
    autocomplete: &AutocompleteEngine,
    code: &str,
    is_asm: bool,
) {
    update_highlight_and_labels_opts(app, autocomplete, code, None, is_asm, true);
}

pub fn populate_filtered_docs_ui(app: &AppWindow, lang: &str, category: &str, query: &str) {
    let docs = crate::autocomplete::intel8080_docs::INTEL_8080_DOCS;
    let theme_id = app.get_active_theme_id();
    let q = query.trim().to_lowercase();
    let has_q = !q.is_empty();

    let doc_items: Vec<DocItem> = docs
        .iter()
        .filter(|doc| {
            if !category.is_empty() && category != "Все" && category != "All" {
                let cat_ru = doc.get_category("ru");
                let cat_en = doc.get_category("en");
                let cat_lower = category.to_lowercase();
                let matches_cat = cat_ru.to_lowercase().contains(&cat_lower)
                    || cat_en.to_lowercase().contains(&cat_lower)
                    || doc.category.to_lowercase().contains(&cat_lower)
                    || doc.category_ru.to_lowercase().contains(&cat_lower);
                if !matches_cat {
                    return false;
                }
            }

            if !has_q {
                return true;
            }

            let desc = crate::autocomplete::intel8080_details::get_detailed_description(doc.name, lang);
            doc.name.to_lowercase().contains(&q)
                || doc.syntax.to_lowercase().contains(&q)
                || doc.get_summary(lang).to_lowercase().contains(&q)
                || doc.get_operands(lang).to_lowercase().contains(&q)
                || doc.flags.to_lowercase().contains(&q)
                || doc.get_category(lang).to_lowercase().contains(&q)
                || desc.to_lowercase().contains(&q)
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
                            char_count: t.char_count,
                            is_whitespace: t.is_whitespace,
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
                operands: SharedString::from(doc.get_operands(lang)),
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
    populate_filtered_docs_ui(app, lang, "Все", "");
    crate::ui::docs_bridge::populate_all_educational_docs(app, lang);
}

pub fn apply_selected_suggestion_to_ui(
    app: &AppWindow,
    idx: usize,
    sugg: &crate::autocomplete::engine::Suggestion,
) {
    app.set_selected_sugg_index(idx as i32);
    app.set_selected_sugg_name(sugg.name.clone());
    app.set_selected_sugg_category(sugg.category.clone());
    app.set_selected_sugg_syntax(sugg.syntax.clone());
    app.set_selected_sugg_summary(sugg.summary.clone());
    app.set_selected_sugg_flags(sugg.flags.clone());
    app.set_selected_sugg_cycles(sugg.cycles.clone());
    app.set_selected_sugg_insert_text(sugg.insert_text.clone());
    app.set_selected_sugg_icon_kind(sugg.icon_kind.clone());
    app.set_selected_sugg_example(sugg.example.clone());
}

pub fn refresh_suggestions_ui(
    app: &AppWindow,
    controller_rc: &Arc<Mutex<IdeController>>,
    query: &str,
    lang: &str,
) {
    let mut controller = controller_rc.lock().unwrap();
    let active_path = controller.file_manager.active_file_path.clone();
    let suggestions = if controller.cached_symbols_path == active_path {
        let symbols = Arc::clone(&controller.cached_user_symbols);
        controller
            .autocomplete
            .get_suggestions_lang_with_symbols(query, &symbols, lang)
    } else {
        let code_opt = active_path
            .as_ref()
            .and_then(|p| controller.tab_buffers.get(p).cloned());
        let line_starts_opt = active_path
            .as_ref()
            .and_then(|p| controller.tab_line_starts.get(p).cloned());
        let current_code = code_opt.unwrap_or_else(|| app.get_code_text().to_string());
        let symbols = Arc::new(
            controller
                .autocomplete
                .get_user_labels_with_starts(&current_code, line_starts_opt.as_deref()),
        );
        controller.cached_symbols_path = active_path;
        controller.cached_user_symbols = Arc::clone(&symbols);
        controller
            .autocomplete
            .get_suggestions_lang_with_symbols(query, &symbols, lang)
    };
    controller.active_suggestions = suggestions.clone();

    if let Some(first) = suggestions.first() {
        apply_selected_suggestion_to_ui(app, 0, first);
    } else {
        app.set_selected_sugg_index(0);
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
    app.set_is_dirty(false);
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
        let p = path.clone();
        ctrl.dirty_tabs.insert(p.clone());
        ctrl.set_tab_buffer(p, new_code.to_string());
    }
}
