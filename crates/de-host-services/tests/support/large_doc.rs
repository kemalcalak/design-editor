//! The large-document fixture (#412, #456): pages of boards of rows, plus
//! what a real file carries — a component page whose master every board
//! instantiates, an image fill and a variable-bound fill per board.

use de_editor_core::EditorState;
use serde_json::{json, Value};

/// A 1 × 1 PNG, so image fills need no file.
const PNG_DATA_URL: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==";

/// Read a size from the environment, else `default`.
pub fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// The fixture's shape. The defaults are #412's 194,600-node document.
pub struct Fixture {
    pub pages: usize,
    pub boards: usize,
    pub rows: usize,
}

impl Fixture {
    /// The #412 shape, overridable through `DE_MEM_PAGES` / `DE_MEM_BOARDS`
    /// / `DE_MEM_ROWS`.
    pub fn from_env() -> Self {
        Self {
            pages: env_usize("DE_MEM_PAGES", 14),
            boards: env_usize("DE_MEM_BOARDS", 100),
            rows: env_usize("DE_MEM_ROWS", 46),
        }
    }

    pub fn page_id(&self, page: usize) -> String {
        format!("page-{page}")
    }

    pub fn board_id(&self, page: usize, board: usize) -> String {
        format!("p{page}b{board}")
    }

    pub fn row_id(&self, page: usize, board: usize, row: usize) -> String {
        format!("p{page}b{board}r{row}")
    }

    /// Nodes on the content pages (boards, rows with two children, the
    /// image and the instance per board) plus the master's three.
    pub fn node_count(&self) -> usize {
        self.pages * self.boards * (3 + self.rows * 3) + 3
    }

    fn row(&self, page: usize, board: usize, row: usize) -> Value {
        let id = self.row_id(page, board, row);
        json!({
            "type": "frame", "id": id, "name": "Row", "width": 360, "height": 40,
            "layout": "horizontal", "gap": 8,
            "children": [
                {"type": "rectangle", "id": format!("{id}-icon"), "width": 24, "height": 24,
                 "fill": [{"type": "solid", "color": "$primary"}], "cornerRadius": 6},
                {"type": "text", "id": format!("{id}-label"),
                 "content": format!("Row {row} of board {board}"),
                 "fontSize": 14, "fill": [{"type": "solid", "color": "#111111"}]}
            ]
        })
    }

    fn board(&self, page: usize, board: usize) -> Value {
        let id = self.board_id(page, board);
        let mut children: Vec<Value> = vec![
            json!({"type": "rectangle", "id": format!("{id}-image"), "width": 360, "height": 120,
                   "fill": [{"type": "image", "url": PNG_DATA_URL, "mode": "fill"}]}),
            json!({"type": "ref", "id": format!("{id}-card"), "ref": "card-master"}),
        ];
        children.extend((0..self.rows).map(|row| self.row(page, board, row)));
        json!({
            "type": "frame", "id": id, "name": format!("Board {board}"),
            "x": (board % 10) * 420, "y": (board / 10) * 2400, "width": 400, "height": 2300,
            "layout": "vertical", "children": children
        })
    }

    fn components_page(&self) -> Value {
        json!({"id": "components", "name": "Components", "children": [{
            "type": "frame", "id": "card-master", "name": "Card", "reusable": true,
            "x": 0, "y": 0, "width": 360, "height": 64, "layout": "horizontal", "padding": 12,
            "fill": [{"type": "solid", "color": "$surface"}],
            "children": [
                {"type": "rectangle", "id": "card-master-dot", "width": 16, "height": 16,
                 "fill": [{"type": "solid", "color": "$primary"}]},
                {"type": "text", "id": "card-master-label", "content": "Component card", "fontSize": 14}
            ]
        }]})
    }

    /// The document as `.de` JSON.
    pub fn json(&self) -> String {
        let mut pages: Vec<Value> = (0..self.pages)
            .map(|page| {
                let boards: Vec<Value> = (0..self.boards).map(|b| self.board(page, b)).collect();
                json!({"id": self.page_id(page), "name": format!("Page {page}"), "children": boards})
            })
            .collect();
        pages.push(self.components_page());
        json!({
            "version": "1.0",
            "variables": {
                "primary": {"type": "color", "value": "#3355FF"},
                "surface": {"type": "color", "value": "#F4F6FB"}
            },
            "pages": pages,
            "children": []
        })
        .to_string()
    }

    /// The editor state a person gets when they open the file.
    pub fn build_state(&self) -> EditorState {
        let json = self.json();
        let doc: jian_ops_schema::PenDocument =
            serde_json::from_str(&json).expect("fixture document parses");
        drop(json);
        EditorState::from_opened_document(doc)
    }
}
