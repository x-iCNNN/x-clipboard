use serde::Deserialize;
use std::sync::OnceLock;
#[derive(Clone, Deserialize)]
pub struct Symbol {
    pub symbol: String,
    pub name: String,
    pub keywords: String,
    pub group: String,
}
pub fn emoji() -> &'static [Symbol] {
    static SYMBOLS: OnceLock<Vec<Symbol>> = OnceLock::new();
    SYMBOLS.get_or_init(|| {
        serde_json::from_str(include_str!("../assets/emoji-zh.json"))
            .expect("bundled emoji catalogue must be valid")
    })
}
pub fn punctuation() -> &'static [Symbol] {
    static SYMBOLS: OnceLock<Vec<Symbol>> = OnceLock::new();
    SYMBOLS.get_or_init(|| {
        [
            ("标点", "间隔号:· 项目符号:• 省略号:… 破折号:— 短横线:– 左直角引号:「 右直角引号:」 左双直角引号:『 右双直角引号:』 左书名号:《 右书名号:》 版权:© 注册商标:® 商标:™"),
            ("数学", "正负号:± 乘号:× 除号:÷ 不等于:≠ 约等于:≈ 小于等于:≤ 大于等于:≥ 无穷大:∞ 平方根:√ 求和:∑ 积分:∫ 圆周率:π 度:° 千分号:‰"),
            ("箭头", "左箭头:← 上箭头:↑ 右箭头:→ 下箭头:↓ 左右箭头:↔ 上下箭头:↕ 双向箭头:⇄ 推导箭头:⇒ 左双箭头:⇐ 上双箭头:⇑ 下双箭头:⇓"),
            ("货币", "人民币:￥ 日元:¥ 美元:$ 欧元:€ 英镑:£ 韩元:₩ 印度卢比:₹ 卢布:₽ 美分:¢"),
            ("几何", "对勾:✓ 粗对勾:✔ 叉号:✕ 实心星:★ 空心星:☆ 实心圆:● 空心圆:○ 实心方块:■ 空心方块:□ 实心菱形:◆ 空心菱形:◇"),
        ].into_iter().flat_map(|(group, entries)| entries.split_whitespace().map(move |entry| {
            let (name, symbol) = entry.split_once(':').expect("bundled symbol entry");
            Symbol { symbol: symbol.into(), name: name.into(), keywords: format!("{name} {group}"), group: group.into() }
        })).collect()
    })
}
