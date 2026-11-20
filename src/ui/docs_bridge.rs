use crate::docs::ascii_table::ASCII_TABLE;
use crate::docs::bdos::{BUFFER_0A_DOCS, CPM_BDOS_DOCS, FCB_STRUCTURE_DOCS};
use crate::docs::flags_jumps::{CODE_PATTERNS, JUMP_CONDITIONS};
use crate::docs::memory_map::MEMORY_SEGMENTS;
use crate::docs::registers_psw::{FLAGS_DOCS, FLAG_RULES, REGISTERS_DOCS};
use crate::syntax::intel8080::highlight_document;
use crate::ui::{
    AppWindow, AsciiItem, BdosItem, BufferFieldItem, CodePatternItem, FcbFieldItem, FlagItem,
    FlagRuleItem, HighlightLineItem, JumpConditionItem, MemoryMapItem, RegisterItem, TokenItem,
};
use slint::{Model, ModelRc, SharedString, VecModel};
use std::rc::Rc;

pub fn populate_all_educational_docs(app: &AppWindow, lang: &str) {
    populate_bdos_list(app, lang, "", "");
    populate_memory_map_list(app, lang);
    populate_registers_and_flags(app, lang);
    populate_ascii_table(app, lang);
    populate_jumps_and_patterns(app, lang);
}

pub fn populate_bdos_list(app: &AppWindow, lang: &str, category: &str, query: &str) {
    let theme_id = app.get_active_theme_id();
    let is_ru = lang == "ru";
    let q = query.trim().to_lowercase();
    let has_q = !q.is_empty();

    let bdos_items: Vec<BdosItem> = CPM_BDOS_DOCS
        .iter()
        .filter(|doc| {
            if !category.is_empty() && category != "Все" && category != "All" {
                let cat_lower = category.to_lowercase();
                let matches_cat = doc.category_ru.to_lowercase().contains(&cat_lower)
                    || doc.category_en.to_lowercase().contains(&cat_lower);
                if !matches_cat {
                    return false;
                }
            }

            if !has_q {
                return true;
            }

            let hex_str = format!("{:02X}h", doc.number);
            doc.name.to_lowercase().contains(&q)
                || hex_str.to_lowercase().contains(&q)
                || doc.number.to_string().contains(&q)
                || doc.title_ru.to_lowercase().contains(&q)
                || doc.title_en.to_lowercase().contains(&q)
                || doc.desc_ru.to_lowercase().contains(&q)
                || doc.desc_en.to_lowercase().contains(&q)
                || doc.notes_ru.to_lowercase().contains(&q)
                || doc.category_ru.to_lowercase().contains(&q)
                || doc.example.to_lowercase().contains(&q)
        })
        .map(|doc| {
            let highlighted = highlight_document(doc.example, theme_id);
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

            let hex_str = format!("{:02X}h", doc.number);

            BdosItem {
                func_num: doc.number as i32,
                func_hex: SharedString::from(hex_str),
                name: SharedString::from(doc.name),
                title: SharedString::from(if is_ru { doc.title_ru } else { doc.title_en }),
                category: SharedString::from(if is_ru {
                    doc.category_ru
                } else {
                    doc.category_en
                }),
                entry_c: SharedString::from(doc.entry_c),
                entry_de: SharedString::from(if is_ru {
                    doc.entry_de
                } else {
                    doc.entry_de_en
                }),
                exit_a: SharedString::from(if is_ru { doc.exit_a } else { doc.exit_a_en }),
                exit_hl: SharedString::from(if is_ru { doc.exit_hl } else { doc.exit_hl_en }),
                description: SharedString::from(if is_ru { doc.desc_ru } else { doc.desc_en }),
                notes: SharedString::from(if is_ru { doc.notes_ru } else { doc.notes_en }),
                raw_snippet: SharedString::from(doc.example),
                example_lines: ModelRc::from(Rc::new(VecModel::from(example_lines))),
            }
        })
        .collect();

    app.set_bdos_total_count(CPM_BDOS_DOCS.len() as i32);
    app.set_bdos_list(ModelRc::from(Rc::new(VecModel::from(bdos_items))));

    // FCB Fields
    let fcb_items: Vec<FcbFieldItem> = FCB_STRUCTURE_DOCS
        .iter()
        .map(|f| FcbFieldItem {
            offset: SharedString::from(f.offset),
            name: SharedString::from(f.name),
            size_str: SharedString::from(f.size),
            desc: SharedString::from(if is_ru { f.desc_ru } else { f.desc_en }),
        })
        .collect();
    app.set_fcb_fields_list(ModelRc::from(Rc::new(VecModel::from(fcb_items))));

    // Buffer 0Ah Fields
    let buf_items: Vec<BufferFieldItem> = BUFFER_0A_DOCS
        .iter()
        .map(|b| BufferFieldItem {
            byte_idx: SharedString::from(b.byte_index),
            name: SharedString::from(b.name),
            set_by: SharedString::from(b.set_by),
            desc: SharedString::from(if is_ru { b.desc_ru } else { b.desc_en }),
        })
        .collect();
    app.set_buffer_fields_list(ModelRc::from(Rc::new(VecModel::from(buf_items))));
}

pub fn populate_memory_map_list(app: &AppWindow, lang: &str) {
    let is_ru = lang == "ru";
    let segments: Vec<MemoryMapItem> = MEMORY_SEGMENTS
        .iter()
        .map(|s| MemoryMapItem {
            start_addr: SharedString::from(s.start_addr),
            end_addr: SharedString::from(s.end_addr),
            name: SharedString::from(if is_ru { s.name_ru } else { s.name_en }),
            category: SharedString::from(if is_ru {
                s.category_ru
            } else {
                s.category_en
            }),
            description: SharedString::from(if is_ru { s.desc_ru } else { s.desc_en }),
            importance: SharedString::from(if is_ru {
                s.importance_ru
            } else {
                s.importance_en
            }),
            is_tpa: s.is_tpa,
        })
        .collect();
    app.set_memory_map_list(ModelRc::from(Rc::new(VecModel::from(segments))));
}

pub fn populate_registers_and_flags(app: &AppWindow, lang: &str) {
    let is_ru = lang == "ru";

    // Registers
    let regs: Vec<RegisterItem> = REGISTERS_DOCS
        .iter()
        .map(|r| RegisterItem {
            name: SharedString::from(r.name),
            bits: r.bits as i32,
            pair_name: SharedString::from(r.pair_name),
            role: SharedString::from(if is_ru { r.role_ru } else { r.role_en }),
            description: SharedString::from(if is_ru { r.desc_ru } else { r.desc_en }),
        })
        .collect();
    app.set_registers_list(ModelRc::from(Rc::new(VecModel::from(regs))));

    // Flags
    let flags: Vec<FlagItem> = FLAGS_DOCS
        .iter()
        .map(|f| FlagItem {
            name: SharedString::from(f.name),
            bit_index: f.bit_index as i32,
            title: SharedString::from(if is_ru { f.title_ru } else { f.title_en }),
            description: SharedString::from(if is_ru {
                f.description_ru
            } else {
                f.description_en
            }),
            when_set: SharedString::from(if is_ru { f.when_set_ru } else { f.when_set_en }),
            when_cleared: SharedString::from(if is_ru {
                f.when_cleared_ru
            } else {
                f.when_cleared_en
            }),
            affected_by: SharedString::from(f.affected_by),
        })
        .collect();
    app.set_flags_list(ModelRc::from(Rc::new(VecModel::from(flags))));

    // Flag Rules
    let rules: Vec<FlagRuleItem> = FLAG_RULES
        .iter()
        .map(|r| FlagRuleItem {
            op_group: SharedString::from(if is_ru {
                r.op_group_ru
            } else {
                r.op_group_en
            }),
            commands: SharedString::from(r.commands),
            effect: SharedString::from(if is_ru { r.effect_ru } else { r.effect_en }),
        })
        .collect();
    app.set_flag_rules_list(ModelRc::from(Rc::new(VecModel::from(rules))));
}

pub fn populate_ascii_table(app: &AppWindow, lang: &str) {
    let is_ru = lang == "ru";
    let items: Vec<AsciiItem> = ASCII_TABLE
        .iter()
        .map(|a| AsciiItem {
            code_dec: a.code as i32,
            code_hex: SharedString::from(a.hex),
            char_display: SharedString::from(a.char_display),
            name: SharedString::from(if is_ru { a.name_ru } else { a.name_en }),
            description: SharedString::from(if is_ru { a.desc_ru } else { a.desc_en }),
            is_cpm_special: a.is_cpm_special,
        })
        .collect();
    app.set_ascii_list(ModelRc::from(Rc::new(VecModel::from(items))));
}

pub fn populate_jumps_and_patterns(app: &AppWindow, lang: &str) {
    let theme_id = app.get_active_theme_id();
    let is_ru = lang == "ru";

    // Jump conditions
    let jumps: Vec<JumpConditionItem> = JUMP_CONDITIONS
        .iter()
        .map(|j| JumpConditionItem {
            condition: SharedString::from(j.condition),
            flag_test: SharedString::from(j.flag_test),
            flag_name: SharedString::from(j.flag_name),
            jump_cmd: SharedString::from(j.jump_cmd),
            call_cmd: SharedString::from(j.call_cmd),
            ret_cmd: SharedString::from(j.ret_cmd),
            meaning: SharedString::from(if is_ru { j.meaning_ru } else { j.meaning_en }),
            scenario: SharedString::from(if is_ru {
                j.scenario_ru
            } else {
                j.scenario_en
            }),
        })
        .collect();
    app.set_jumps_list(ModelRc::from(Rc::new(VecModel::from(jumps))));

    // Practical Code Patterns with Syntax Highlighting
    let patterns: Vec<CodePatternItem> = CODE_PATTERNS
        .iter()
        .map(|pat| {
            let highlighted = highlight_document(pat.code_snippet, theme_id);
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

            CodePatternItem {
                title: SharedString::from(if is_ru { pat.title_ru } else { pat.title_en }),
                category: SharedString::from(if is_ru {
                    pat.category_ru
                } else {
                    pat.category_en
                }),
                description: SharedString::from(if is_ru {
                    pat.desc_ru
                } else {
                    pat.desc_en
                }),
                raw_snippet: SharedString::from(pat.code_snippet),
                example_lines: ModelRc::from(Rc::new(VecModel::from(example_lines))),
            }
        })
        .collect();
    app.set_patterns_list(ModelRc::from(Rc::new(VecModel::from(patterns))));
}

pub fn search_all_docs(app: &AppWindow, lang: &str, query: &str) {
    let q = query.trim().to_lowercase();
    let is_ru = lang == "ru";

    if q.is_empty() {
        let cat = app.get_docs_selected_category();
        crate::ui::editor_bridge::populate_filtered_docs_ui(app, lang, cat.as_str(), "");
        let bdos_cat = app.get_bdos_selected_category();
        populate_bdos_list(app, lang, bdos_cat.as_str(), "");
        populate_ascii_table(app, lang);
        return;
    }

    let all_label = if is_ru { "Все" } else { "All" };
    app.set_docs_selected_category(SharedString::from(all_label));
    app.set_bdos_selected_category(SharedString::from(all_label));

    // 1. Filter Intel 8080 Instructions & Directives
    crate::ui::editor_bridge::populate_filtered_docs_ui(app, lang, all_label, query);

    // 2. Filter CP/M BDOS Calls
    populate_bdos_list(app, lang, all_label, query);

    // 3. Filter ASCII Table
    let filtered_ascii: Vec<AsciiItem> = ASCII_TABLE
        .iter()
        .filter(|a| {
            a.code.to_string().contains(&q)
                || a.hex.to_lowercase().contains(&q)
                || a.char_display.to_lowercase().contains(&q)
                || a.name_ru.to_lowercase().contains(&q)
                || a.name_en.to_lowercase().contains(&q)
                || a.desc_ru.to_lowercase().contains(&q)
                || a.desc_en.to_lowercase().contains(&q)
        })
        .map(|a| AsciiItem {
            code_dec: a.code as i32,
            code_hex: SharedString::from(a.hex),
            char_display: SharedString::from(a.char_display),
            name: SharedString::from(if is_ru { a.name_ru } else { a.name_en }),
            description: SharedString::from(if is_ru { a.desc_ru } else { a.desc_en }),
            is_cpm_special: a.is_cpm_special,
        })
        .collect();
    app.set_ascii_list(ModelRc::from(Rc::new(VecModel::from(filtered_ascii))));

    // Automatically switch active module to the first one that has matching results if current module has 0
    let cur_module = app.get_docs_active_module();
    let has_docs = app.get_docs_list().row_count() > 0;
    let has_bdos = app.get_bdos_list().row_count() > 0;
    let has_ascii = app.get_ascii_list().row_count() > 0;

    let cur_has_matches = match cur_module {
        0 => has_docs,
        1 => has_bdos,
        4 => has_ascii,
        _ => false,
    };

    if !cur_has_matches {
        if has_docs {
            app.set_docs_active_module(0);
        } else if has_bdos {
            app.set_docs_active_module(1);
        } else if has_ascii {
            app.set_docs_active_module(4);
        }
    }
}
