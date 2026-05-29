use godot::prelude::*;
use godot::classes::StyleBoxFlat;

pub fn attribute_color(attr: &str) -> Color {
    match attr {
        "STR" => Color::from_html("#e74c3c").unwrap(),
        "AGI" => Color::from_html("#2ecc71").unwrap(),
        "INT" => Color::from_html("#3498db").unwrap(),
        _ => Color::from_rgba(0.5, 0.5, 0.5, 1.0),
    }
}

pub fn attribute_stylebox(attr: &str) -> Gd<StyleBoxFlat> {
    let mut style = StyleBoxFlat::new_gd();
    let mut color = attribute_color(attr);
    color.a = 0.3;
    style.set_bg_color(color);
    style
}

pub fn ultimate_stylebox() -> Gd<StyleBoxFlat> {
    let mut style = StyleBoxFlat::new_gd();
    style.set_bg_color(Color::from_rgba(0.0, 0.0, 0.0, 0.0));
    let purple = Color::from_html("#9b59b6").unwrap();
    style.set_border_color(purple);
    style.set_border_width_all(3);
    style
}

pub fn format_ability_tooltip(info: &godot::prelude::VarDictionary) -> GString {
    let name = info.get("name").map(|v| v.to::<GString>().to_string()).unwrap_or_default();
    let desc = info.get("description").map(|v| v.to::<GString>().to_string()).unwrap_or_default();
    let mana = info.get("mana_cost").map(|v| v.to::<GString>().to_string()).unwrap_or_default();
    let cd = info.get("cooldown").map(|v| v.to::<GString>().to_string()).unwrap_or_default();
    let range = info.get("cast_range").map(|v| v.to::<f32>()).unwrap_or(0.0);
    let targeting = info.get("targeting").map(|v| v.to::<GString>().to_string()).unwrap_or_default();

    let text = format!(
        "{name}\nCast Range: {range:.0} | Targeting: {targeting}\nMana: {mana} | Cooldown: {cd}\n\n{desc}"
    );
    GString::from(text.as_str())
}
