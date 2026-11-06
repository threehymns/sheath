//! Vector drawables and adaptive icons, rewritten as SVG for Rune's rasterizer.
//!
//! Android's vector grammar is close to SVG: paths, groups with a pivot-based transform, and
//! paint. An adaptive icon is a background layer and a foreground layer drawn in that order on
//! one canvas.

use super::axml::{Node, Value};
use super::table::{Table, Value as Resource};

pub(super) fn vector(root: &Node, table: &Table) -> Result<String, String> {
    let (width, height) = viewport(root).ok_or("the vector has no viewport")?;
    let mut svg = open(width, height);
    for child in &root.children {
        element(child, table, &mut svg);
    }
    svg.push_str("</svg>");
    Ok(svg)
}

pub(super) fn adaptive(layers: &[&Node], table: &Table) -> Result<String, String> {
    let mut width = 0.0f32;
    let mut height = 0.0f32;
    for layer in layers {
        if let Some((layer_width, layer_height)) = viewport(layer) {
            width = width.max(layer_width);
            height = height.max(layer_height);
        }
    }
    if width <= 0.0 || height <= 0.0 {
        return Err("the adaptive icon has no sized layers".into());
    }
    let mut svg = open(width, height);
    for layer in layers {
        for child in &layer.children {
            element(child, table, &mut svg);
        }
    }
    svg.push_str("</svg>");
    Ok(svg)
}

fn open(width: f32, height: f32) -> String {
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {width} {height}">"#)
}

fn viewport(node: &Node) -> Option<(f32, f32)> {
    Some((
        number(node, "viewportWidth")?,
        number(node, "viewportHeight")?,
    ))
}

fn element(node: &Node, table: &Table, out: &mut String) {
    match node.name.as_str() {
        "path" => path(node, table, out),
        "group" => group(node, table, out),
        _ => {
            for child in &node.children {
                element(child, table, out);
            }
        }
    }
}

/// Android applies a group's scale, then its rotation, then its translation, the rotation and
/// scale about the pivot. SVG applies a transform list right to left, so the same order reads
/// left to right with the pivot moved into place first and back afterwards.
fn group(node: &Node, table: &Table, out: &mut String) {
    let pivot_x = number(node, "pivotX").unwrap_or(0.0);
    let pivot_y = number(node, "pivotY").unwrap_or(0.0);
    let translate_x = number(node, "translateX").unwrap_or(0.0);
    let translate_y = number(node, "translateY").unwrap_or(0.0);
    let scale_x = number(node, "scaleX").unwrap_or(1.0);
    let scale_y = number(node, "scaleY").unwrap_or(1.0);
    let rotation = number(node, "rotation").unwrap_or(0.0);
    let mut transform = String::new();
    if translate_x != 0.0 || translate_y != 0.0 {
        transform.push_str(&format!("translate({translate_x} {translate_y}) "));
    }
    if pivot_x != 0.0 || pivot_y != 0.0 {
        transform.push_str(&format!("translate({pivot_x} {pivot_y}) "));
    }
    if rotation != 0.0 {
        transform.push_str(&format!("rotate({rotation}) "));
    }
    if scale_x != 1.0 || scale_y != 1.0 {
        transform.push_str(&format!("scale({scale_x} {scale_y}) "));
    }
    if pivot_x != 0.0 || pivot_y != 0.0 {
        transform.push_str(&format!("translate({} {}) ", -pivot_x, -pivot_y));
    }
    out.push_str("<g");
    if !transform.is_empty() {
        out.push_str(&format!(r#" transform="{}""#, transform.trim_end()));
    }
    out.push('>');
    for child in &node.children {
        element(child, table, out);
    }
    out.push_str("</g>");
}

fn path(node: &Node, table: &Table, out: &mut String) {
    let Some(data) = text(node, "pathData") else {
        return;
    };
    out.push_str(r#"<path d=""#);
    out.push_str(data);
    out.push('"');
    if let Some((color, opacity)) = paint(node, table, "fillColor", "fillAlpha") {
        out.push_str(&format!(r#" fill="{color}""#));
        if opacity < 1.0 {
            out.push_str(&format!(r#" fill-opacity="{opacity}""#));
        }
    } else {
        out.push_str(r#" fill="none""#);
    }
    if integer(node, "fillType") == Some(1) {
        out.push_str(r#" fill-rule="evenodd""#);
    }
    if let Some((color, opacity)) = paint(node, table, "strokeColor", "strokeAlpha") {
        let width = number(node, "strokeWidth").unwrap_or(0.0);
        if width > 0.0 {
            out.push_str(&format!(r#" stroke="{color}" stroke-width="{width}""#));
            if opacity < 1.0 {
                out.push_str(&format!(r#" stroke-opacity="{opacity}""#));
            }
            match integer(node, "strokeLineCap") {
                Some(1) => out.push_str(r#" stroke-linecap="round""#),
                Some(2) => out.push_str(r#" stroke-linecap="square""#),
                _ => {}
            }
            match integer(node, "strokeLineJoin") {
                Some(1) => out.push_str(r#" stroke-linejoin="round""#),
                Some(2) => out.push_str(r#" stroke-linejoin="bevel""#),
                _ => {}
            }
        }
    }
    out.push_str("/>");
}

/// A paint color and its opacity. The color's own alpha and the drawable's alpha multiply, and
/// a fully transparent paint is no paint at all.
fn paint(node: &Node, table: &Table, color: &str, alpha: &str) -> Option<(String, f32)> {
    let argb = match node.attr(color)? {
        Value::Int(value) => *value as u32,
        Value::Ref(id) => match table.resolve(*id)? {
            Resource::Color(value) => value,
            Resource::Path(_) => return None,
        },
        _ => return None,
    };
    let multiply = number(node, alpha).unwrap_or(1.0);
    let opacity = ((argb >> 24) & 0xff) as f32 / 255.0 * multiply;
    if opacity <= 0.0 {
        return None;
    }
    Some((format!("#{:06x}", argb & 0x00ff_ffff), opacity.min(1.0)))
}

fn text<'a>(node: &'a Node, name: &str) -> Option<&'a str> {
    match node.attr(name)? {
        Value::Str(text) => Some(text),
        _ => None,
    }
}

fn number(node: &Node, name: &str) -> Option<f32> {
    match node.attr(name)? {
        Value::Float(number) => Some(*number),
        Value::Int(number) => Some(*number as f32),
        _ => None,
    }
}

fn integer(node: &Node, name: &str) -> Option<i32> {
    match node.attr(name)? {
        Value::Int(number) => Some(*number),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::preview::apk::axml::Attr;

    fn attr(name: &str, value: Value) -> Attr {
        Attr {
            name: name.into(),
            value,
        }
    }

    #[test]
    fn a_group_pivots_rotates_and_scales_like_android_and_paths_keep_their_paint() {
        let path = Node {
            name: "path".into(),
            attrs: vec![
                attr("pathData", Value::Str("M0,0L10,10".into())),
                attr("fillColor", Value::Int(0xff00_ff00u32 as i32)),
                attr("strokeColor", Value::Int(0x80ff_0000u32 as i32)),
                attr("strokeWidth", Value::Float(2.0)),
                attr("strokeLineCap", Value::Int(1)),
            ],
            children: Vec::new(),
        };
        let group = Node {
            name: "group".into(),
            attrs: vec![
                attr("pivotX", Value::Float(50.0)),
                attr("pivotY", Value::Float(50.0)),
                attr("translateX", Value::Float(4.0)),
                attr("translateY", Value::Float(5.0)),
                attr("scaleX", Value::Float(0.7)),
                attr("scaleY", Value::Float(0.7)),
                attr("rotation", Value::Float(-25.0)),
            ],
            children: vec![path],
        };
        let root = Node {
            name: "vector".into(),
            attrs: vec![
                attr("viewportWidth", Value::Float(108.0)),
                attr("viewportHeight", Value::Float(108.0)),
            ],
            children: vec![group],
        };
        let svg = vector(&root, &Table::empty()).unwrap();
        assert!(
            svg.contains(
                r#"transform="translate(4 5) translate(50 50) rotate(-25) scale(0.7 0.7) translate(-50 -50)""#
            ),
            "{svg}"
        );
        assert!(svg.contains(r##"fill="#00ff00""##), "{svg}");
        assert!(
            svg.contains(r##"stroke="#ff0000" stroke-width="2" stroke-opacity="0.5019608""##),
            "{svg}"
        );
        assert!(svg.contains(r#"stroke-linecap="round""#), "{svg}");
    }
}
