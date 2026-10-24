//! Public re-exports for the `editor_ui_state` spine.

pub use super::chrome::{
    DesignMdRequest, EmbedHost, FileAction, PencilCursorStyle, PencilSmoothing, RecentFile,
    ThemeMode, ThemePresetIo, UpdateStatus, RECENT_FILE_CAP,
};
pub use super::git_panel::{
    CloneField, CloneFormState, CommitDiffPatch, CommitDiffSummary, CommitDiffView,
    GitBranchPickerMode, GitCandidateFile, GitCommitSummary, GitDiffTarget, GitDiffView,
    GitFileEntry, GitOverflowView, GitPanelAction, GitPanelState, MergeConflictRow,
    MergeResolveFile, MergeResolveState,
};
pub use super::groups::{
    AssetCenterTab, CustomPrompt, DesignMdPanelState, PreviewState, PromptCenterFocus,
    PromptCenterState, PromptFilter, SceneFilter, SceneTemplateCenterState, SceneTemplateFocus,
    SizeToggleState, StyleImportState,
};
pub use super::home::{
    EntrySurface, HomeCopyTarget, HomeFamily, HomeHit, HomeSize, HomeState, HomeTemplate,
    RecentView, TaskDraft,
};
pub use super::pickers::{
    CanvasDropIndicator, CanvasOverlayLine, CanvasOverlayRect, CompositingPickerTarget,
    EffectParamFocus, FontPickerPurpose, LayerContextMenuState, MissingFontSurface,
    PageRenameState, PreviewDeviceKind, VariableRowFocus,
};
pub use super::slides_panel_state::{
    LeftPanelTab, SlidesDrag, SlidesPanelState, SlidesPanelTarget, CHAT_TAB_MIN_WIDTH,
    LAYER_PANEL_MAX_WIDTH, LAYER_PANEL_MIN_WIDTH,
};
pub use super::update_state::{
    UpdateAction, UpdateFailure, UpdateModalButton, UpdateModalState, UpdatePhase,
};
pub use super::workspace::{
    WorkspaceHit, WorkspacePhase, WorkspaceState, WorkspaceView, WORKSPACE_DECK_STRIP_H,
    WORKSPACE_ENTER_MS, WORKSPACE_HEADER_H, WORKSPACE_TOOLBAR_H,
};

// `Locale` is the i18n locale enum — dependency-free + wasm-clean, so
// it lives in `de-i18n` and re-exports cleanly into the state layer.
pub use de_i18n::Locale;

pub use crate::property_panel_state::{
    BooleanOp, ExportFormat, FillType, FlexLayout, ImageAdjustmentField, ImageFillMode,
    PaddingEditMode, PropertyTab,
};

// What the LayerPanel right-click context menu is acting on — the
// canonical definition is `ui_draft::LayerContextTarget` (it backs
// the inline-rename draft too). Re-exported so UI code that
// references a context target has one import path.
pub use crate::ui_draft::LayerContextTarget;
