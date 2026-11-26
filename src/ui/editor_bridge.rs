use crate::autocomplete::engine::AutocompleteEngine;
use crate::syntax::intel8080::{HighlightToken, highlight_document_range_with_offsets};
use crate::ui::controller::IdeController;
use crate::ui::{AppWindow, DocItem, HighlightLineItem, LabelItem, SuggestionItem, TokenItem};
use slint::{Model, ModelRc, SharedString, VecModel};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

const SMALL_ASM_FULL_HIGHLIGHT_MAX_LINES: usize = 350;
const HIGHLIGHT_PAD_LINES: usize = 55;

struct PersistentHighlightState {
    model: Rc<VecModel<HighlightLineItem>>,
    buffer_start: usize,
    buffer_end: usize,
    theme_id: i32,
}

thread_local! {
    static GUTTER_CACHE: Cell<(usize, usize)> = const { Cell::new((usize::MAX, usize::MAX)) };
    static HIGHLIGHT_STATE: RefCell<Option<PersistentHighlightState>> = const { RefCell::new(None) };
}

pub fn reset_editor_ui_caches() {
    GUTTER_CACHE.with(|c| c.set((usize::MAX, usize::MAX)));
    HIGHLIGHT_STATE.with(|s| *s.borrow_mut() = None);
}

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

/// Coalesces trailing plain whitespace tokens into preceding non-italic tokens to cut Slint Text element count.
fn coalesce_line_tokens(tokens: Vec<HighlightToken>) -> Vec<TokenItem> {
    let mut out: Vec<TokenItem> = Vec::with_capacity(tokens.len());
    for tok in tokens {
        if tok.is_whitespace
            && let Some(prev) = out.last_mut()
            && !prev.is_italic
        {
            let mut merged = String::with_capacity(prev.text.len() + tok.text.len());
            merged.push_str(prev.text.as_str());
            merged.push_str(tok.text.as_str());
            prev.text = SharedString::from(merged);
            prev.char_count += tok.char_count;
            continue;
        }
        out.push(TokenItem {
            text: tok.text,
            color: tok.color,
            is_italic: tok.is_italic,
            char_count: tok.char_count,
            is_whitespace: tok.is_whitespace,
        });
    }
    out
}

fn to_highlight_line_item(line: crate::syntax::intel8080::HighlightLine) -> HighlightLineItem {
    let token_models = coalesce_line_tokens(line.tokens);
    HighlightLineItem {
        line_number: line.line_number,
        tokens: ModelRc::from(Rc::new(VecModel::from(token_models))),
    }
}

pub fn update_gutter_line_numbers(
    app: &AppWindow,
    total_lines: usize,
    scroll_line: usize,
    visible_count: usize,
) -> (usize, usize) {
    let total = total_lines.max(1);
    let (start_line, end_line) = if total <= SMALL_ASM_FULL_HIGHLIGHT_MAX_LINES {
        (0, total)
    } else {
        let start = scroll_line
            .saturating_sub(HIGHLIGHT_PAD_LINES)
            .min(total.saturating_sub(1));
        let count = (visible_count + HIGHLIGHT_PAD_LINES * 2).clamp(90, 200);
        (start, (start + count).min(total))
    };

    let already_cached = GUTTER_CACHE.with(|c| c.get() == (start_line, end_line))
        && app.get_gutter_start_line() == start_line as i32;
    if already_cached {
        return (start_line, end_line);
    }
    GUTTER_CACHE.with(|c| c.set((start_line, end_line)));

    let mut numbers_str = String::with_capacity((end_line - start_line) * 7);
    for i in (start_line + 1)..=end_line {
        push_usize(&mut numbers_str, i);
        numbers_str.push('\n');
    }
    app.set_gutter_start_line(start_line as i32);
    app.set_line_numbers_text(SharedString::from(numbers_str));
    (start_line, end_line)
}

#[inline]
fn count_lines_fast(code: &str, line_starts: Option<&[usize]>) -> usize {
    if code.is_empty() {
        1
    } else if let Some(starts) = line_starts {
        starts.len().max(1)
    } else {
        code.as_bytes().iter().filter(|&&b| b == b'\n').count() + 1
    }
}

#[derive(Clone, Copy)]
pub struct ViewportRenderRequest {
    pub total_lines: Option<usize>,
    pub scroll_line: usize,
    pub visible_count: usize,
    pub is_asm: bool,
    pub force_reparse: bool,
}

fn render_highlight_window(
    app: &AppWindow,
    code: &str,
    line_starts: Option<&[usize]>,
    total_lines: usize,
    req: ViewportRenderRequest,
) -> (usize, usize) {
    if !req.is_asm || code.is_empty() || app.get_is_large_file_mode() {
        HIGHLIGHT_STATE.with(|s| *s.borrow_mut() = None);
        app.set_highlight_lines(ModelRc::default());
        return (0, total_lines.min(1));
    }

    let theme_id = app.get_active_theme_id();
    let (buffered_start, buffered_end) = if total_lines <= SMALL_ASM_FULL_HIGHLIGHT_MAX_LINES {
        (0, total_lines)
    } else {
        let start = req
            .scroll_line
            .saturating_sub(HIGHLIGHT_PAD_LINES)
            .min(total_lines.saturating_sub(1));
        let count = (req.visible_count + HIGHLIGHT_PAD_LINES * 2).clamp(90, 200);
        (start, (start + count).min(total_lines))
    };

    if buffered_end <= buffered_start {
        HIGHLIGHT_STATE.with(|s| *s.borrow_mut() = None);
        app.set_highlight_lines(ModelRc::default());
        return (buffered_start, buffered_start);
    }

    let handled_incrementally = HIGHLIGHT_STATE.with(|cell| {
        let mut slot = cell.borrow_mut();
        let Some(state) = slot.as_mut() else {
            return false;
        };
        if req.force_reparse || state.theme_id != theme_id {
            return false;
        }
        if state.buffer_start == buffered_start && state.buffer_end == buffered_end {
            if app.get_highlight_start_line() != buffered_start as i32 {
                app.set_highlight_start_line(buffered_start as i32);
            }
            return true;
        }
        let overlaps = buffered_end > state.buffer_start && state.buffer_end > buffered_start;
        if !overlaps {
            return false;
        }

        let old_start = state.buffer_start;
        let old_end = state.buffer_end;

        if buffered_start > old_start {
            let drop_front = (buffered_start - old_start).min(state.model.row_count());
            for _ in 0..drop_front {
                state.model.remove(0);
            }
        }
        let clipped_old_start = buffered_start.max(old_start);
        if old_end > buffered_end {
            let keep = buffered_end.saturating_sub(clipped_old_start);
            while state.model.row_count() > keep {
                let last = state.model.row_count() - 1;
                state.model.remove(last);
            }
        }
        if buffered_start < old_start {
            let prefix_count = old_start - buffered_start;
            let prefix_lines = highlight_document_range_with_offsets(
                code,
                line_starts,
                theme_id,
                buffered_start,
                prefix_count,
            );
            for (idx, line) in prefix_lines.into_iter().enumerate() {
                state.model.insert(idx, to_highlight_line_item(line));
            }
        }
        if buffered_end > old_end {
            let suffix_count = buffered_end - old_end;
            let suffix_lines = highlight_document_range_with_offsets(
                code,
                line_starts,
                theme_id,
                old_end,
                suffix_count,
            );
            for line in suffix_lines {
                state.model.push(to_highlight_line_item(line));
            }
        }

        state.buffer_start = buffered_start;
        state.buffer_end = buffered_start + state.model.row_count();
        app.set_highlight_start_line(buffered_start as i32);
        true
    });

    if !handled_incrementally {
        let lines = highlight_document_range_with_offsets(
            code,
            line_starts,
            theme_id,
            buffered_start,
            buffered_end - buffered_start,
        );
        let rendered_count = lines.len();
        let line_models: Vec<HighlightLineItem> =
            lines.into_iter().map(to_highlight_line_item).collect();
        let model = Rc::new(VecModel::from(line_models));
        app.set_highlight_start_line(buffered_start as i32);
        app.set_highlight_lines(ModelRc::from(model.clone()));
        HIGHLIGHT_STATE.with(|cell| {
            *cell.borrow_mut() = Some(PersistentHighlightState {
                model,
                buffer_start: buffered_start,
                buffer_end: buffered_start + rendered_count,
                theme_id,
            });
        });
        return (buffered_start, buffered_start + rendered_count);
    }

    (buffered_start, buffered_end)
}

pub fn update_viewport_highlight_for_total(
    app: &AppWindow,
    code: &str,
    line_starts: Option<&[usize]>,
    req: ViewportRenderRequest,
) -> (usize, usize) {
    let total_lines = req
        .total_lines
        .unwrap_or_else(|| count_lines_fast(code, line_starts))
        .max(1);
    let gutter_range =
        update_gutter_line_numbers(app, total_lines, req.scroll_line, req.visible_count);

    if !req.is_asm || code.is_empty() || app.get_is_large_file_mode() {
        HIGHLIGHT_STATE.with(|s| *s.borrow_mut() = None);
        app.set_highlight_lines(ModelRc::default());
        return gutter_range;
    }

    render_highlight_window(app, code, line_starts, total_lines, req)
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
    let total_lines = count_lines_fast(code, line_starts);

    if !is_asm || app.get_is_large_file_mode() {
        if refresh_line_numbers {
            update_gutter_line_numbers(app, total_lines, scroll_line, visible_count);
        }
        HIGHLIGHT_STATE.with(|s| *s.borrow_mut() = None);
        app.set_highlight_lines(ModelRc::default());
        app.set_label_list(ModelRc::default());
        return Vec::new();
    }

    let req = ViewportRenderRequest {
        total_lines: Some(total_lines),
        scroll_line,
        visible_count,
        is_asm,
        force_reparse: true,
    };
    if refresh_line_numbers {
        update_viewport_highlight_for_total(app, code, line_starts, req);
    } else {
        render_highlight_window(app, code, line_starts, total_lines, req);
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
    let total_lines = count_lines_fast(code, line_starts);
    let req = ViewportRenderRequest {
        total_lines: Some(total_lines),
        scroll_line,
        visible_count,
        is_asm,
        force_reparse: true,
    };
    if refresh_line_numbers {
        update_viewport_highlight_for_total(app, code, line_starts, req);
    } else if is_asm && !app.get_is_large_file_mode() {
        render_highlight_window(app, code, line_starts, total_lines, req);
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
    let is_ru = lang == "ru";
    let mut docs_cat = app.get_docs_selected_category().to_string();
    if docs_cat.is_empty() || docs_cat == "Все" || docs_cat == "All" {
        docs_cat = if is_ru { "Все" } else { "All" }.to_string();
        app.set_docs_selected_category(SharedString::from(&docs_cat));
    }
    let mut bdos_cat = app.get_bdos_selected_category().to_string();
    if bdos_cat.is_empty() || bdos_cat == "Все" || bdos_cat == "All" {
        bdos_cat = if is_ru { "Все" } else { "All" }.to_string();
        app.set_bdos_selected_category(SharedString::from(&bdos_cat));
    }
    let query = app.get_docs_search_query().to_string();

    populate_filtered_docs_ui(app, lang, &docs_cat, &query);
    if query.trim().is_empty() {
        crate::ui::docs_bridge::populate_all_educational_docs(app, lang);
        if bdos_cat != "Все" && bdos_cat != "All" {
            crate::ui::docs_bridge::populate_bdos_list(app, lang, &bdos_cat, "");
        }
    } else {
        crate::ui::docs_bridge::search_all_docs(app, lang, &query);
    }
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
    let suggestions = if app.get_is_large_file_mode() || controller.large_window.is_some() {
        controller
            .autocomplete
            .get_suggestions_lang_with_symbols(query, &[], lang)
    } else if controller.cached_symbols_path == active_path {
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

#[cfg(test)]
mod tests {
    use super::coalesce_line_tokens;
    use crate::syntax::intel8080::{HighlightToken, TokenType};
    use slint::{Color, SharedString};

    #[test]
    fn test_coalesce_line_tokens_merges_trailing_spaces_on_non_italic() {
        let white = Color::from_rgb_u8(200, 200, 200);
        let tokens = vec![
            HighlightToken {
                text: SharedString::from("mov"),
                token_type: TokenType::MnemonicDataTransfer,
                color: white,
                is_italic: false,
                char_count: 3,
                is_whitespace: false,
            },
            HighlightToken {
                text: SharedString::from("   "),
                token_type: TokenType::Text,
                color: white,
                is_italic: false,
                char_count: 3,
                is_whitespace: true,
            },
            HighlightToken {
                text: SharedString::from("equ"),
                token_type: TokenType::DirectiveDef,
                color: white,
                is_italic: true,
                char_count: 3,
                is_whitespace: false,
            },
            HighlightToken {
                text: SharedString::from(" "),
                token_type: TokenType::Text,
                color: white,
                is_italic: false,
                char_count: 1,
                is_whitespace: true,
            },
        ];

        let coalesced = coalesce_line_tokens(tokens);
        assert_eq!(coalesced.len(), 3);
        assert_eq!(coalesced[0].text.as_str(), "mov   ");
        assert_eq!(coalesced[0].char_count, 6);
        assert_eq!(coalesced[1].text.as_str(), "equ");
        assert!(coalesced[1].is_italic);
        assert_eq!(coalesced[2].text.as_str(), " ");
    }
}

pub fn record_editor_mutation(
    app: &AppWindow,
    ctrl: &mut IdeController,
    new_code: &str,
    cursor_pos: i32,
    is_group: bool,
) {
    ensure_active_file_or_create_draft(app, ctrl);
    if ctrl.large_window.is_some() {
        ctrl.apply_window_mutation(app, new_code);
        return;
    }
    ctrl.undo_manager
        .record_edit(new_code, cursor_pos, is_group);
    if let Some(path) = &ctrl.file_manager.active_file_path {
        let p = path.clone();
        ctrl.dirty_tabs.insert(p.clone());
        ctrl.set_tab_buffer(p, new_code.to_string());
    }
}

pub fn refresh_active_editor_highlight_and_labels(
    app: &AppWindow,
    ctrl: &IdeController,
    is_asm: bool,
    refresh_line_numbers: bool,
) -> Vec<crate::autocomplete::extractor::UserSymbol> {
    if let Some(active_path) = &ctrl.file_manager.active_file_path
        && let Some(buf) = ctrl.tab_buffers.get(active_path)
    {
        let line_starts = ctrl.tab_line_starts.get(active_path).map(Vec::as_slice);
        return update_highlight_and_labels_opts(
            app,
            &ctrl.autocomplete,
            buf.as_str(),
            line_starts,
            is_asm,
            refresh_line_numbers,
        );
    }
    let code = app.get_code_text().to_string();
    update_highlight_and_labels_opts(
        app,
        &ctrl.autocomplete,
        &code,
        None,
        is_asm,
        refresh_line_numbers,
    )
}
