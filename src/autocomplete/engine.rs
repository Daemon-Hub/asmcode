use super::extractor::{SymbolKind, UserSymbol, extract_user_labels};
use super::intel8080_docs::{INTEL_8080_DOCS, InstructionDoc};
use rayon::prelude::*;

#[derive(Clone, Debug)]
pub struct Suggestion {
    pub name: String,
    pub category: String,
    pub syntax: String,
    pub summary: String,
    pub flags: String,
    pub cycles: String,
    pub insert_text: String,
    pub icon_kind: String,
    pub example: String,
}

#[derive(Clone, Debug)]
pub struct AutocompleteEngine {
    docs: &'static [InstructionDoc],
}

impl AutocompleteEngine {
    pub fn new() -> Self {
        Self {
            docs: INTEL_8080_DOCS,
        }
    }

    #[allow(dead_code)]
    pub fn get_suggestions(&self, current_word: &str, full_source: &str) -> Vec<Suggestion> {
        self.get_suggestions_lang(current_word, full_source, "en")
    }

    pub fn get_suggestions_lang(
        &self,
        current_word: &str,
        full_source: &str,
        lang: &str,
    ) -> Vec<Suggestion> {
        let mut results = Vec::new();
        let query = current_word.trim();
        let is_ru = lang == "ru";
        let query_upper = query.to_ascii_uppercase();

        let doc_results: Vec<Suggestion> = if self.docs.len() > 16 {
            self.docs
                .par_iter()
                .filter_map(|doc| {
                    if query.is_empty() || doc.name.to_ascii_uppercase().starts_with(&query_upper) {
                        Some(Suggestion {
                            name: doc.name.to_string(),
                            category: doc.get_category(lang).to_string(),
                            syntax: doc.syntax.to_string(),
                            summary: doc.get_summary(lang).to_string(),
                            flags: doc.flags.to_string(),
                            cycles: doc.cycles.to_string(),
                            insert_text: doc.name.to_lowercase(),
                            icon_kind: doc.get_icon_kind().to_string(),
                            example: doc.get_example().to_string(),
                        })
                    } else {
                        None
                    }
                })
                .collect()
        } else {
            self.docs
                .iter()
                .filter_map(|doc| {
                    if query.is_empty() || doc.name.to_ascii_uppercase().starts_with(&query_upper) {
                        Some(Suggestion {
                            name: doc.name.to_string(),
                            category: doc.get_category(lang).to_string(),
                            syntax: doc.syntax.to_string(),
                            summary: doc.get_summary(lang).to_string(),
                            flags: doc.flags.to_string(),
                            cycles: doc.cycles.to_string(),
                            insert_text: doc.name.to_lowercase(),
                            icon_kind: doc.get_icon_kind().to_string(),
                            example: doc.get_example().to_string(),
                        })
                    } else {
                        None
                    }
                })
                .collect()
        };

        let mut user_results = Vec::new();
        let user_symbols = extract_user_labels(full_source);
        for symbol in &user_symbols {
            if query.is_empty() || symbol.name.to_ascii_uppercase().starts_with(&query_upper) {
                let (category, summary, example) = match symbol.kind {
                    SymbolKind::Constant => {
                        let cat = if is_ru {
                            "Константа (EQU)".to_string()
                        } else {
                            "Constant (EQU)".to_string()
                        };
                        let sum = if is_ru {
                            format!(
                                "Символьная константа EQU ({}, строка {})",
                                symbol.detail, symbol.line_number
                            )
                        } else {
                            format!(
                                "Symbolic EQU constant ({}, line {})",
                                symbol.detail, symbol.line_number
                            )
                        };
                        let ex = format!(
                            "mvi a, {}    ; Загрузка константы {}",
                            symbol.name, symbol.name
                        );
                        (cat, sum, ex)
                    }
                    SymbolKind::Data => {
                        let cat = if is_ru {
                            "Данные / Буфер".to_string()
                        } else {
                            "Data / Buffer".to_string()
                        };
                        let sum = if is_ru {
                            format!(
                                "Определение данных в памяти ({}, строка {})",
                                symbol.detail, symbol.line_number
                            )
                        } else {
                            format!(
                                "Memory data definition ({}, line {})",
                                symbol.detail, symbol.line_number
                            )
                        };
                        let ex =
                            format!("lxi d, {}    ; Адрес буфера {}", symbol.name, symbol.name);
                        (cat, sum, ex)
                    }
                    SymbolKind::Code => {
                        let cat = if is_ru {
                            "Метка кода".to_string()
                        } else {
                            "Code Label".to_string()
                        };
                        let sum = if is_ru {
                            format!(
                                "Метка перехода / подпрограммы (строка {})",
                                symbol.line_number
                            )
                        } else {
                            format!("Code label / jump target (line {})", symbol.line_number)
                        };
                        let ex = format!(
                            "jmp {}        ; Переход на метку {}",
                            symbol.name, symbol.name
                        );
                        (cat, sum, ex)
                    }
                };

                user_results.push(Suggestion {
                    name: symbol.name.clone(),
                    category,
                    syntax: symbol.name.clone(),
                    summary,
                    flags: if is_ru {
                        "Нет".to_string()
                    } else {
                        "None".to_string()
                    },
                    cycles: "-".to_string(),
                    insert_text: symbol.name.clone(),
                    icon_kind: "label".to_string(),
                    example,
                });
            }
        }

        if query.is_empty() {
            results.extend(doc_results);
            results.extend(user_results);
        } else {
            // Prioritize exact match first
            let mut exact_matches = Vec::new();
            let mut other_matches = Vec::new();

            for s in doc_results.into_iter().chain(user_results) {
                if s.name.eq_ignore_ascii_case(query) {
                    exact_matches.push(s);
                } else {
                    other_matches.push(s);
                }
            }

            results.extend(exact_matches);
            results.extend(other_matches);
        }

        if !query.is_empty() && results.is_empty() {
            let upper = query.to_ascii_uppercase();
            for doc in self.docs {
                if doc.name.contains(&upper)
                    || doc.get_summary(lang).to_ascii_uppercase().contains(&upper)
                {
                    results.push(Suggestion {
                        name: doc.name.to_string(),
                        category: doc.get_category(lang).to_string(),
                        syntax: doc.syntax.to_string(),
                        summary: doc.get_summary(lang).to_string(),
                        flags: doc.flags.to_string(),
                        cycles: doc.cycles.to_string(),
                        insert_text: doc.name.to_lowercase(),
                        icon_kind: doc.get_icon_kind().to_string(),
                        example: doc.get_example().to_string(),
                    });
                }
            }
        }

        results
    }

    pub fn get_user_labels(&self, full_source: &str) -> Vec<UserSymbol> {
        extract_user_labels(full_source)
    }

    #[allow(dead_code)]
    pub fn find_doc(&self, instruction: &str) -> Option<&'static InstructionDoc> {
        let upper = instruction.trim().to_ascii_uppercase();
        self.docs.iter().find(|d| d.name == upper)
    }
}
