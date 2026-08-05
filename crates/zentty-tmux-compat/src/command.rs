use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Command {
    SplitWindow,
    SendKeys,
    SelectPane,
    SelectWindow,
    KillPane,
    KillWindow,
    ListPanes,
    ListWindows,
    DisplayMessage,
    RenameWindow,
    SelectLayout,
    ResizePane,
    NewSession,
    NewWindow,
    WaitFor,
    ShowOptions,
    LastPane,
    SaveBuffer,
    ShowBuffer,
    SetBuffer,
    LoadBuffer,
    CapturePane,
    Popup,
}

impl Command {
    /// Canonicalizes a source-supported tmux command or alias.
    ///
    /// # Errors
    ///
    /// Returns [`CommandError`] rather than silently accepting an unknown
    /// command as the Swift implementation currently does.
    pub fn parse(value: &str) -> Result<Self, CommandError> {
        match value.to_ascii_lowercase().as_str() {
            "split-window" | "splitw" => Ok(Self::SplitWindow),
            "send-keys" | "send" => Ok(Self::SendKeys),
            "select-pane" | "selectp" => Ok(Self::SelectPane),
            "select-window" | "selectw" => Ok(Self::SelectWindow),
            "kill-pane" | "killp" => Ok(Self::KillPane),
            "kill-window" | "killw" => Ok(Self::KillWindow),
            "list-panes" | "lsp" => Ok(Self::ListPanes),
            "list-windows" | "lsw" => Ok(Self::ListWindows),
            "display-message" | "display" => Ok(Self::DisplayMessage),
            "rename-window" | "renamew" => Ok(Self::RenameWindow),
            "select-layout" | "selectl" => Ok(Self::SelectLayout),
            "resize-pane" | "resizep" => Ok(Self::ResizePane),
            "new-session" | "new" => Ok(Self::NewSession),
            "new-window" | "neww" => Ok(Self::NewWindow),
            "wait-for" | "wait" => Ok(Self::WaitFor),
            "show" | "show-options" | "show-option" | "showw" | "show-window-options" => {
                Ok(Self::ShowOptions)
            }
            "last-pane" | "lastp" => Ok(Self::LastPane),
            "save-buffer" | "saveb" => Ok(Self::SaveBuffer),
            "show-buffer" | "showb" => Ok(Self::ShowBuffer),
            "set-buffer" | "setb" => Ok(Self::SetBuffer),
            "load-buffer" | "loadb" => Ok(Self::LoadBuffer),
            "capture-pane" | "capturep" => Ok(Self::CapturePane),
            "popup" => Ok(Self::Popup),
            _ => Err(CommandError(value.to_owned())),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandError(String);

impl fmt::Display for CommandError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "unsupported tmux compatibility command: {}",
            self.0
        )
    }
}

impl std::error::Error for CommandError {}
