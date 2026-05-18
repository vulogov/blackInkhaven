use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;

#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum Direction {
    Up,
    Down,
}

pub fn run(project: &Path, node_path: &str, direction: Direction) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;

    let cfg = Config::load(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;
    let h = Hierarchy::load(&store)?;

    let node = h
        .find_by_path(node_path)
        .ok_or_else(|| Error::Store(format!("node not found: `{node_path}`")))?;
    let siblings = h.children_of(node.parent_id);
    let pos = siblings
        .iter()
        .position(|s| s.id == node.id)
        .ok_or_else(|| Error::Store(format!("`{node_path}` not in its parent's children")))?;

    let other_pos = match direction {
        Direction::Up => {
            if pos == 0 {
                return Err(Error::Store(format!("`{node_path}` is already first")));
            }
            pos - 1
        }
        Direction::Down => {
            if pos + 1 >= siblings.len() {
                return Err(Error::Store(format!("`{node_path}` is already last")));
            }
            pos + 1
        }
    };
    let other = siblings[other_pos];
    store.swap_siblings(&h, node.id, other.id)?;

    eprintln!(
        "moved `{}` {} (swapped with `{}`)",
        node_path,
        match direction {
            Direction::Up => "up",
            Direction::Down => "down",
        },
        other.slug
    );
    Ok(())
}
