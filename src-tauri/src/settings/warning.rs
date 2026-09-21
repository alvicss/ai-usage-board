//! 檔案：settings/warning.rs
//! 用途：定義「設定檔哪裡有問題」的警告格式。
//!       畫面依 kind 從翻譯檔取出白話說明，合法範圍、選項、型別用結構化欄位（minimum／maximum／choices／value_type）
//!       交給畫面依語言組字（中文「1～120」、英文「1 to 120」，FR-17）；log 則用 log_text() 的中文句子。
//! 誰會用到：settings/load.rs、settings/validate.rs 產生警告；commands.rs 回傳給畫面；lib.rs 寫進 log。

use serde::Serialize;

/// 警告種類（序列化成 snake_case，畫面用 `settings_warning.<kind>` 當翻譯 key）。
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WarningKind {
    /// 整個檔案格式錯誤（TOML 語法錯），本次全部使用預設值
    InvalidFile,
    /// 欄位型別不對，例如數字欄位填了文字
    WrongType,
    /// 數字超出允許範圍
    OutOfRange,
    /// 文字欄位不是允許的選項之一
    InvalidChoice,
    /// 顏色不是 #RRGGBB
    InvalidColor,
    /// 卡片邊框不是 default／none／#RRGGBB
    InvalidBorder,
    /// CLI 路徑不存在，或不是 .exe／.cmd
    InvalidCliPath,
    /// 設定檔格式版本不支援
    UnsupportedSchema,
}

/// 一筆設定警告。
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SettingsWarning {
    /// 出問題的欄位，例如 "general.refresh_interval_minutes"；整檔錯誤時為 "settings.toml"
    pub field: String,
    /// 警告種類
    pub kind: WarningKind,
    /// 整檔錯誤時，問題大約在第幾行（從 1 起算）；其他情況為 None
    pub line: Option<usize>,
    /// 合法值的中文說明，例如 "1～120"、"auto、light、dark"、"#RRGGBB"（log 用；畫面改用下面的結構化欄位）
    pub expected: String,
    /// 本次改用的值（整檔錯誤時為空白）
    pub fallback: String,
    /// 數字範圍的下限（out_of_range 才有）
    pub minimum: Option<i64>,
    /// 數字範圍的上限（out_of_range 才有）
    pub maximum: Option<i64>,
    /// 允許的選項（invalid_choice 才有），例如 ["auto", "light", "dark"]
    pub choices: Vec<String>,
    /// 型別不對時，正確的型別（wrong_type 才有）：string／integer／boolean／string_list／table
    pub value_type: Option<String>,
}

impl SettingsWarning {
    /// 建立一筆警告（範圍、選項、型別等結構化欄位先留空，需要時用 with_* 補上）。
    pub fn new(field: &str, kind: WarningKind, expected: &str, fallback: &str) -> Self {
        Self {
            field: field.to_string(),
            kind,
            line: None,
            expected: expected.to_string(),
            fallback: fallback.to_string(),
            minimum: None,
            maximum: None,
            choices: Vec::new(),
            value_type: None,
        }
    }

    /// 補上數字範圍（畫面依語言組成「1～120」或「1 to 120」）。
    pub fn with_range(mut self, minimum: i64, maximum: i64) -> Self {
        self.minimum = Some(minimum);
        self.maximum = Some(maximum);
        self
    }

    /// 補上允許的選項（畫面依語言用「、」或「, 」串起來）。
    pub fn with_choices(mut self, choices: &[&str]) -> Self {
        let mut list = Vec::new();
        for choice in choices {
            list.push(choice.to_string());
        }
        self.choices = list;
        self
    }

    /// 補上正確的型別代號（畫面翻譯成「整數」或 "whole number"）。
    pub fn with_value_type(mut self, value_type: &str) -> Self {
        self.value_type = Some(value_type.to_string());
        self
    }

    /// 給 log 用的中文句子（不含任何帳號資訊）。
    pub fn log_text(&self) -> String {
        match self.kind {
            WarningKind::InvalidFile => {
                let line = self.line.unwrap_or(1);
                let field_hint = if self.field == "settings.toml" {
                    String::new()
                } else {
                    format!("（欄位 {}）", self.field)
                };
                format!(
                    "settings.toml 第 {line} 行附近格式不正確{field_hint}，本次全部使用預設值；原檔保留，按設定畫面的「儲存」之前不會覆寫"
                )
            }
            WarningKind::WrongType => format!(
                "{} 的型別不對（應為 {}），已改用預設值 {}",
                self.field, self.expected, self.fallback
            ),
            WarningKind::OutOfRange => format!(
                "{} 必須介於 {}，已改用預設值 {}",
                self.field, self.expected, self.fallback
            ),
            WarningKind::InvalidChoice => format!(
                "{} 只能是 {}，已改用預設值 {}",
                self.field, self.expected, self.fallback
            ),
            WarningKind::InvalidColor => format!(
                "{} 必須是 #RRGGBB 格式的顏色，已改用預設值 {}",
                self.field, self.fallback
            ),
            WarningKind::InvalidBorder => format!(
                "{} 只能是 default、none 或 #RRGGBB，已改用預設值 {}",
                self.field, self.fallback
            ),
            WarningKind::InvalidCliPath => format!(
                "{} 必須指向存在的 .exe 或 .cmd 檔案，已改用自動偵測",
                self.field
            ),
            WarningKind::UnsupportedSchema => format!(
                "{} 不支援（應為 {}），已改用 {}",
                self.field, self.expected, self.fallback
            ),
        }
    }
}
