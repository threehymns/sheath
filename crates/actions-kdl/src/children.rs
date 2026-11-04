use kdl::{KdlDocument, KdlNode};

fn parent_index(doc: &KdlDocument, parent: &str) -> Option<usize> {
    doc.nodes()
        .iter()
        .position(|node| node.name().value() == parent)
}

// Rewrites the one child and keeps the parent's indentation, so the file's comments and layout
// survive a save.
pub fn write_child(doc: &mut KdlDocument, parent: &str, name: &str, body: &str) {
    let Some(index) = parent_index(doc, parent) else {
        let text = format!("\n{parent} {{\n    {body}\n}}\n");
        if let Ok(parsed) = text.parse::<KdlDocument>()
            && let Some(node) = parsed.nodes().first()
        {
            doc.nodes_mut().push(node.clone());
        }
        return;
    };
    let has_children = doc.nodes()[index].children().is_some();
    let existing = doc.nodes()[index]
        .children()
        .into_iter()
        .flat_map(|kids| kids.nodes())
        .find(|node| node.name().value() == name)
        .and_then(|node| node.format().map(|format| format.leading.clone()));
    let leading = existing.unwrap_or_else(|| "\n    ".to_owned());
    if !has_children {
        let text = format!("{leading}{parent} {{\n    {body}\n}}\n");
        if let Ok(parsed) = text.parse::<KdlDocument>()
            && let Some(node) = parsed.nodes().first()
        {
            doc.nodes_mut()[index] = node.clone();
        }
        return;
    }
    let Some(children) = doc.nodes_mut()[index].children_mut().as_mut() else {
        return;
    };
    let text = format!("{leading}{body}\n");
    let Ok(node) = text.parse::<KdlNode>() else {
        return;
    };
    if let Some(slot) = children
        .nodes_mut()
        .iter_mut()
        .find(|child| child.name().value() == name)
    {
        *slot = node;
    } else {
        children.nodes_mut().push(node);
    }
}

pub fn remove_child(doc: &mut KdlDocument, parent: &str, name: &str) {
    let Some(index) = parent_index(doc, parent) else {
        return;
    };
    if let Some(children) = doc.nodes_mut()[index].children_mut().as_mut() {
        children
            .nodes_mut()
            .retain(|node| node.name().value() != name);
    }
}
