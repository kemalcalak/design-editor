//! The commands the palette can run (#395): a label key, the chord that
//! runs the same command, and English keywords so a search finds a command
//! by what it does as well as by its label. Hosts map each command onto the
//! call its shortcut or menu item makes.

/// Every palette command. `ALL` is the order the palette lists them in
/// when nothing is typed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaletteCommand {
    FindLayers,
    Undo,
    Redo,
    Duplicate,
    Delete,
    SelectAll,
    Group,
    Ungroup,
    CreateComponent,
    RenameLayers,
    BooleanUnion,
    BooleanSubtract,
    BooleanIntersect,
    BooleanExclude,
    Flatten,
    OutlineStroke,
    FlipHorizontal,
    FlipVertical,
    BringForward,
    SendBackward,
    ToolSelect,
    ToolFrame,
    ToolRectangle,
    ToolEllipse,
    ToolLine,
    ToolPolygon,
    ToolPen,
    ToolPencil,
    ToolText,
    ToolNote,
    ToolHand,
    Preview,
    Present,
    FitAll,
    TogglePanel,
    ToggleCodePanel,
    ToggleVariables,
    ToggleDesignMd,
    ComponentBrowser,
    TogglePrototypeLinks,
    TogglePixelGridSnap,
    ImportFigma,
    ImportHtml,
    Settings,
    NewChat,
    GitPanel,
    NewFile,
    OpenFile,
    Save,
    SaveAs,
    ExportImage,
}

/// One row of the command table.
pub struct CommandSpec {
    pub label_key: &'static str,
    /// macOS spelling (`⌘⇧Z`); empty when the command has no chord.
    pub mac: &'static str,
    /// Windows / Linux spelling (`Ctrl+Shift+Z`).
    pub other: &'static str,
    /// Extra English words a search matches.
    pub keywords: &'static str,
}

const fn spec(
    label_key: &'static str,
    mac: &'static str,
    other: &'static str,
    keywords: &'static str,
) -> CommandSpec {
    CommandSpec {
        label_key,
        mac,
        other,
        keywords,
    }
}

impl PaletteCommand {
    pub const ALL: [PaletteCommand; 51] = [
        PaletteCommand::FindLayers,
        PaletteCommand::Undo,
        PaletteCommand::Redo,
        PaletteCommand::Duplicate,
        PaletteCommand::Delete,
        PaletteCommand::SelectAll,
        PaletteCommand::Group,
        PaletteCommand::Ungroup,
        PaletteCommand::CreateComponent,
        PaletteCommand::RenameLayers,
        PaletteCommand::BooleanUnion,
        PaletteCommand::BooleanSubtract,
        PaletteCommand::BooleanIntersect,
        PaletteCommand::BooleanExclude,
        PaletteCommand::Flatten,
        PaletteCommand::OutlineStroke,
        PaletteCommand::FlipHorizontal,
        PaletteCommand::FlipVertical,
        PaletteCommand::BringForward,
        PaletteCommand::SendBackward,
        PaletteCommand::ToolSelect,
        PaletteCommand::ToolFrame,
        PaletteCommand::ToolRectangle,
        PaletteCommand::ToolEllipse,
        PaletteCommand::ToolLine,
        PaletteCommand::ToolPolygon,
        PaletteCommand::ToolPen,
        PaletteCommand::ToolPencil,
        PaletteCommand::ToolText,
        PaletteCommand::ToolNote,
        PaletteCommand::ToolHand,
        PaletteCommand::Preview,
        PaletteCommand::Present,
        PaletteCommand::FitAll,
        PaletteCommand::TogglePanel,
        PaletteCommand::ToggleCodePanel,
        PaletteCommand::ToggleVariables,
        PaletteCommand::ToggleDesignMd,
        PaletteCommand::ComponentBrowser,
        PaletteCommand::TogglePrototypeLinks,
        PaletteCommand::TogglePixelGridSnap,
        PaletteCommand::ImportFigma,
        PaletteCommand::ImportHtml,
        PaletteCommand::Settings,
        PaletteCommand::NewChat,
        PaletteCommand::GitPanel,
        PaletteCommand::NewFile,
        PaletteCommand::OpenFile,
        PaletteCommand::Save,
        PaletteCommand::SaveAs,
        PaletteCommand::ExportImage,
    ];

    pub fn spec(self) -> CommandSpec {
        use PaletteCommand as C;
        match self {
            C::FindLayers => spec("palette.cmd.findLayers", "⌘F", "Ctrl+F", "search layer"),
            C::Undo => spec("toolbar.undo", "⌘Z", "Ctrl+Z", "history back"),
            C::Redo => spec("toolbar.redo", "⌘⇧Z", "Ctrl+Shift+Z", "history again"),
            C::Duplicate => spec("common.duplicate", "⌘D", "Ctrl+D", "copy clone"),
            C::Delete => spec("common.delete", "⌫", "Delete", "remove"),
            C::SelectAll => spec("menu.selectAll", "⌘A", "Ctrl+A", "everything"),
            C::Group => spec("menu.group", "⌘G", "Ctrl+G", "wrap"),
            C::Ungroup => spec("menu.ungroup", "⌘⇧G", "Ctrl+Shift+G", "unwrap"),
            C::CreateComponent => spec(
                "layerMenu.createComponent",
                "⌘⌥K",
                "Ctrl+Alt+K",
                "master reusable symbol",
            ),
            C::RenameLayers => spec(
                "layerMenu.renameLayers",
                "⌘⇧R",
                "Ctrl+Shift+R",
                "ai names semantic",
            ),
            C::BooleanUnion => spec(
                "layerMenu.booleanUnion",
                "⌘⌥U",
                "Ctrl+Alt+U",
                "boolean merge combine",
            ),
            C::BooleanSubtract => spec(
                "layerMenu.booleanSubtract",
                "⌘⌥S",
                "Ctrl+Alt+S",
                "boolean minus",
            ),
            C::BooleanIntersect => spec(
                "layerMenu.booleanIntersect",
                "⌘⌥I",
                "Ctrl+Alt+I",
                "boolean overlap",
            ),
            C::BooleanExclude => spec(
                "layerMenu.booleanExclude",
                "⌘⌥X",
                "Ctrl+Alt+X",
                "boolean xor difference",
            ),
            C::Flatten => spec("path.flatten", "⌘E", "Ctrl+E", "path vector"),
            C::OutlineStroke => spec("path.outlineStroke", "⌘⇧O", "Ctrl+Shift+O", "path vector"),
            C::FlipHorizontal => spec("palette.cmd.flipHorizontal", "⇧H", "Shift+H", "mirror"),
            C::FlipVertical => spec("palette.cmd.flipVertical", "⇧V", "Shift+V", "mirror"),
            C::BringForward => spec("palette.cmd.bringForward", "]", "]", "order z raise"),
            C::SendBackward => spec("palette.cmd.sendBackward", "[", "[", "order z lower"),
            C::ToolSelect => spec("toolbar.select", "V", "V", "tool move pointer"),
            C::ToolFrame => spec("toolbar.frame", "F", "F", "tool board artboard"),
            C::ToolRectangle => spec("palette.cmd.toolRectangle", "R", "R", "tool shape box"),
            C::ToolEllipse => spec(
                "palette.cmd.toolEllipse",
                "O",
                "O",
                "tool shape circle oval",
            ),
            C::ToolLine => spec("palette.cmd.toolLine", "L", "L", "tool shape"),
            C::ToolPolygon => spec("palette.cmd.toolPolygon", "Y", "Y", "tool shape star"),
            C::ToolPen => spec("palette.cmd.toolPen", "P", "P", "tool path vector"),
            C::ToolPencil => spec(
                "palette.cmd.toolPencil",
                "⇧P",
                "Shift+P",
                "tool draw freehand",
            ),
            C::ToolText => spec("toolbar.text", "T", "T", "tool type"),
            C::ToolNote => spec("palette.cmd.toolNote", "N", "N", "tool sticky comment"),
            C::ToolHand => spec("toolbar.hand", "H", "H", "tool pan"),
            C::Preview => spec("palette.cmd.preview", "⌘P", "Ctrl+P", "play prototype run"),
            C::Present => spec("workspace.present", "⌘↩", "Ctrl+Enter", "slides slideshow"),
            C::FitAll => spec("statusbar.focusContent", "", "", "zoom fit view"),
            C::TogglePanel => spec(
                "palette.cmd.togglePanel",
                "⌘\\",
                "Ctrl+\\",
                "layers sidebar",
            ),
            C::ToggleCodePanel => spec("topbar.codePanel", "⌘⇧C", "Ctrl+Shift+C", "export"),
            C::ToggleVariables => spec(
                "palette.cmd.variables",
                "⌘⇧V",
                "Ctrl+Shift+V",
                "tokens themes",
            ),
            C::ToggleDesignMd => spec("menu.designMdPanel", "⌘⇧D", "Ctrl+Shift+D", "design md"),
            C::ComponentBrowser => spec(
                "componentBrowser.title",
                "⌘⇧K",
                "Ctrl+Shift+K",
                "components kit library insert",
            ),
            C::TogglePrototypeLinks => spec("menu.showPrototypeLinks", "", "", "prototype"),
            C::TogglePixelGridSnap => spec("menu.snapPixelGrid", "", "", "pixel grid"),
            C::ImportFigma => spec("topbar.importFigma", "⌘⇧F", "Ctrl+Shift+F", "fig"),
            C::ImportHtml => spec("palette.cmd.importHtml", "⌘⇧H", "Ctrl+Shift+H", "web page"),
            C::Settings => spec(
                "topbar.agentsAndMcp",
                "⌘,",
                "Ctrl+,",
                "settings preferences",
            ),
            C::NewChat => spec("ai.newChat", "⌘T", "Ctrl+T", "ai agent"),
            C::GitPanel => spec("menu.gitPanel", "", "", "version commit"),
            C::NewFile => spec("menu.new", "", "", "document"),
            C::OpenFile => spec("menu.open", "⌘O", "Ctrl+O", "document"),
            C::Save => spec("common.save", "⌘S", "Ctrl+S", "document"),
            C::SaveAs => spec("menu.saveAs", "⌘⇧S", "Ctrl+Shift+S", "document"),
            C::ExportImage => spec("menu.exportImage", "⌘⇧P", "Ctrl+Shift+P", "png export"),
        }
    }

    /// The chord as this platform spells it; empty when there is none.
    pub fn chord(self) -> &'static str {
        let spec = self.spec();
        if cfg!(any(target_os = "windows", target_os = "linux")) {
            spec.other
        } else {
            spec.mac
        }
    }

    /// Commands only the desktop runner can carry out (file dialogs, the
    /// Git session, chat tabs); a widget host hands them back to it.
    pub fn needs_runner(self) -> bool {
        matches!(
            self,
            PaletteCommand::NewChat
                | PaletteCommand::GitPanel
                | PaletteCommand::NewFile
                | PaletteCommand::OpenFile
                | PaletteCommand::Save
                | PaletteCommand::SaveAs
                | PaletteCommand::ExportImage
        )
    }
}
